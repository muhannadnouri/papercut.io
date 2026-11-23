"""Reproduce Kokoro's duration Squeeze with the standalone OpenVINO EP on Intel GPU."""
import json
import tempfile
from pathlib import Path

import numpy as np
import onnx
import onnxruntime as ort
import onnxruntime_ep_openvino as ep
from onnx import TensorProto, helper


def model(explicit_axis):
    x = helper.make_tensor_value_info("duration", TensorProto.INT64, [1, "N"])
    y = helper.make_tensor_value_info("squeezed", TensorProto.INT64, ["N"])
    inputs = ["duration"]
    initializers = []
    if explicit_axis:
        inputs.append("axes")
        initializers.append(helper.make_tensor("axes", TensorProto.INT64, [1], [0]))
    node = helper.make_node("Squeeze", inputs, ["squeezed"], name="/Squeeze")
    graph = helper.make_graph([node], "kokoro-duration", [x], [y], initializers)
    result = helper.make_model(graph, opset_imports=[helper.make_opsetid("", 14)])
    result.ir_version = 8
    onnx.checker.check_model(result)
    return result.SerializeToString()


def main():
    ort.register_execution_provider_library(ep.get_ep_name(), ep.get_library_path())
    gpu = next(d for d in ort.get_ep_devices() if d.ep_metadata.get("ov_device") == "GPU")
    print(gpu.ep_metadata.get("FULL_DEVICE_NAME"), gpu.ep_metadata.get("GPU_DEVICE_ID"))
    x = np.arange(4, dtype=np.int64).reshape(1, 4)
    with tempfile.TemporaryDirectory() as tmp:
        for explicit_axis in (False, True):
            opts = ort.SessionOptions()
            opts.enable_profiling = True
            opts.profile_file_prefix = str(Path(tmp) / "trace")
            opts.add_provider_for_devices([gpu], {})
            try:
                session = ort.InferenceSession(model(explicit_axis), opts)
            except Exception as error:
                if explicit_axis or "rank().is_static()" not in str(error):
                    raise
                print("Implicit axes reproduced rank().is_static()")
                continue
            output, = session.run(None, {"duration": x})
            np.testing.assert_array_equal(output, x[0])
            events = json.loads(Path(session.end_profiling()).read_text())
            assert any(e.get("args", {}).get("provider") == ep.get_ep_name() for e in events)
            print(f"explicit_axis={explicit_axis}: GPU execution and output verified")


if __name__ == "__main__":
    main()
