# Alignment model attribution

This notice describes the auxiliary alignment pack, not a translation engine.
Papercut's MIT license does not replace the licenses of these model assets.

## Model and tokenizer

Base: BERT multilingual base (cased), published by Google Research.
The Hugging Face model card was written by the Hugging Face team.

- Base model: https://huggingface.co/google-bert/bert-base-multilingual-cased
- Original release and model license statement:
  https://github.com/google-research/bert/blob/eedf5716ce1268e56f0a50264a88cafad334ac61/README.md
- License: Apache License, Version 2.0; full text is LICENSE-APACHE-2.0.txt.

The upstream export and quantization were published by cstr:
https://huggingface.co/cstr/awesome-align-onnx-int8/tree/e803a2f501474044099b539160aaca461976ecd0

The selected artifact is an ONNX INT8 export of the first eight mBERT layers.
Papercut uses the pinned upstream graph and tokenizer bytes unchanged. Papercut
has not trained, fine-tuned, quantized or re-exported this pack. The original
upstream README.md accompanies the installed pack and retains its model card,
provenance statements and citations. The exporter does not identify an exact
base-checkpoint revision or reproducible export recipe. Its specific eight-layer
description conflicts with its generic claim of no architecture change; this
notice does not claim equivalence to an untouched twelve-layer base model.

No standalone upstream NOTICE was supplied in the inspected export/base Hub
inventories or the linked original BERT repository. This is Papercut-maintained
attribution, not an upstream NOTICE, a new license or an endorsement.

## Research attribution

BERT: Jacob Devlin, Ming-Wei Chang, Kenton Lee and Kristina Toutanova (2018),
https://arxiv.org/abs/1810.04805

Awesome-align: Zi-Yi Dou and Graham Neubig (2021),
https://github.com/neulab/awesome-align/tree/5f150d45bbe51e167daf0a84abebaeb07c3323d1

No Awesome-align extraction source or Python package is included in this pack.
Its BSD-3-Clause source license is separate from the model's Apache-2.0 terms.
Runtime libraries and Rust dependencies retain their own licenses/notices;
these model notices do not replace their distribution requirements.
