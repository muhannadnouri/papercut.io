//! Pinned model assets and identities; model capacity is independent of numerical precision.
use super::ModelSelection;

const SMALL_REVISION: &str = "614241f622f53c4eeff9890bdc4f31cfecc418b3";

#[derive(Clone, Copy)]
pub(super) struct Asset {
    pub path: &'static str,
    pub upstream_path: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
}

impl ModelSelection {
    pub(super) fn model_name(self) -> &'static str {
        match self {
            Self::Float32 | Self::DynamicInt8 => "multilingual-e5-small",
            Self::BaseFloat32 => "multilingual-e5-base",
            Self::LargeFloat32 => "multilingual-e5-large",
        }
    }

    fn revision(self) -> &'static str {
        match self {
            Self::Float32 | Self::DynamicInt8 => SMALL_REVISION,
            Self::BaseFloat32 => "d128750597153bb5987e10b1c3493a34e5a4502a",
            Self::LargeFloat32 => "3d7cfbdacd47fdda877c5cd8a79fbcc4f2a574f3",
        }
    }

    pub(crate) fn dimensions(self) -> usize {
        match self {
            Self::Float32 | Self::DynamicInt8 => 384,
            Self::BaseFloat32 => 768,
            Self::LargeFloat32 => 1024,
        }
    }

    pub(super) fn cache_key(self) -> String {
        let precision = if self == Self::DynamicInt8 {
            "dynamic-int8"
        } else {
            "float32"
        };
        format!("{}/{}/{precision}", self.model_name(), self.revision())
    }

    pub(super) fn asset_url(self, asset: &Asset) -> String {
        format!(
            "https://huggingface.co/intfloat/{}/resolve/{}/{}",
            self.model_name(),
            self.revision(),
            asset.upstream_path
        )
    }

    pub(super) fn encoder_key(self) -> String {
        // Keep Small identity unchanged; the app index also hashes every asset, including external weights.
        format!(
            "{}/{}/mean-e5-prefix-v1/{}/512/single-text",
            self.cache_key(),
            self.assets()[4].sha256,
            self.dimensions()
        )
    }

    pub(super) fn assets(self) -> Vec<Asset> {
        let (upstream_path, bytes, sha256) = match self {
            Self::Float32 => (
                "onnx/model.onnx",
                470_268_510,
                "ca456c06b3a9505ddfd9131408916dd79290368331e7d76bb621f1cba6bc8665",
            ),
            Self::DynamicInt8 => (
                "onnx/model_qint8_avx512_vnni.onnx",
                118_346_824,
                "dd476dd0c2514e9b9be83aeb3853fac0763e0bdf4a71645407587d77c48a2d88",
            ),
            Self::BaseFloat32 => return BASE_ASSETS.to_vec(),
            Self::LargeFloat32 => return LARGE_ASSETS.to_vec(),
        };
        vec![
            Asset {
                path: "config.json",
                upstream_path: "config.json",
                bytes: 655,
                sha256: "69137736cab8b8903a07fe8afaafdda25aac55415a12a55d1bffa9f581abf959",
            },
            Asset {
                path: "tokenizer.json",
                upstream_path: "tokenizer.json",
                bytes: 17_082_730,
                sha256: "0b44a9d7b51c3c62626640cda0e2c2f70fdacdc25bbbd68038369d14ebdf4c39",
            },
            Asset {
                path: "special_tokens_map.json",
                upstream_path: "special_tokens_map.json",
                bytes: 167,
                sha256: "d05497f1da52c5e09554c0cd874037a083e1dc1b9cfd48034d1c717f1afc07a7",
            },
            Asset {
                path: "tokenizer_config.json",
                upstream_path: "tokenizer_config.json",
                bytes: 443,
                sha256: "a1d6bc8734a6f635dc158508bef000f8e2e5a759c7d92f984b2c86e5ff53425b",
            },
            Asset {
                path: "onnx/model.onnx",
                upstream_path,
                bytes,
                sha256,
            },
        ]
    }

    pub(super) fn installed_bytes(self) -> u64 {
        self.assets().iter().map(|asset| asset.bytes).sum()
    }
}

const BASE_ASSETS: [Asset; 5] = [
    Asset {
        path: "config.json",
        upstream_path: "config.json",
        bytes: 694,
        sha256: "9dab198f24c8c0879e481cf7822005d5ecbceedbacb390ffafa594e28d31bac4",
    },
    Asset {
        path: "tokenizer.json",
        upstream_path: "tokenizer.json",
        bytes: 17082660,
        sha256: "62c24cdc13d4c9952d63718d6c9fa4c287974249e16b7ade6d5a85e7bbb75626",
    },
    Asset {
        path: "special_tokens_map.json",
        upstream_path: "special_tokens_map.json",
        bytes: 280,
        sha256: "06e405a36dfe4b9604f484f6a1e619af1a7f7d09e34a8555eb0b77b66318067f",
    },
    Asset {
        path: "tokenizer_config.json",
        upstream_path: "tokenizer_config.json",
        bytes: 418,
        sha256: "efb5c0d09722e5fe59a462cd2a9976ee216d55b037597d997cd3fe833216da15",
    },
    Asset {
        path: "onnx/model.onnx",
        upstream_path: "onnx/model.onnx",
        bytes: 1110059084,
        sha256: "84a4d426f7e87a6bf5bf195f0bae2c4a7d15f675b23ca96f42fab8326d7a77aa",
    },
];

const LARGE_ASSETS: [Asset; 6] = [
    Asset {
        path: "config.json",
        upstream_path: "config.json",
        bytes: 690,
        sha256: "e42d98d0c211928d788467aa951000391017f647c76ca8ce2ef6dd8d0c9ce5ca",
    },
    Asset {
        path: "tokenizer.json",
        upstream_path: "tokenizer.json",
        bytes: 17082660,
        sha256: "62c24cdc13d4c9952d63718d6c9fa4c287974249e16b7ade6d5a85e7bbb75626",
    },
    Asset {
        path: "special_tokens_map.json",
        upstream_path: "special_tokens_map.json",
        bytes: 280,
        sha256: "06e405a36dfe4b9604f484f6a1e619af1a7f7d09e34a8555eb0b77b66318067f",
    },
    Asset {
        path: "tokenizer_config.json",
        upstream_path: "tokenizer_config.json",
        bytes: 418,
        sha256: "efb5c0d09722e5fe59a462cd2a9976ee216d55b037597d997cd3fe833216da15",
    },
    Asset {
        path: "onnx/model.onnx",
        upstream_path: "onnx/model.onnx",
        bytes: 545850,
        sha256: "bb5a52503a3ef35247f5b5ae6c473aaae60505dd3ffaef56d7b69e2f84683c05",
    },
    Asset {
        path: "onnx/model.onnx_data",
        upstream_path: "onnx/model.onnx_data",
        bytes: 2235363328,
        sha256: "0cf1883fee81c63819a44e2ba0efa51d4043d9759685a4ebebbde97e0623d15c",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selections_pin_separate_assets_dimensions_and_preserve_small_identity() {
        let variants = [
            ModelSelection::Float32,
            ModelSelection::DynamicInt8,
            ModelSelection::BaseFloat32,
            ModelSelection::LargeFloat32,
        ];
        let keys = variants.map(ModelSelection::encoder_key);
        assert_eq!(
            keys.iter().collect::<std::collections::HashSet<_>>().len(),
            4
        );
        assert_eq!(
            variants.map(ModelSelection::dimensions),
            [384, 384, 768, 1024]
        );
        assert_eq!(
            variants.map(ModelSelection::installed_bytes),
            [487_352_505, 135_430_819, 1_127_143_136, 2_252_993_226]
        );
        assert_eq!(
            ModelSelection::Float32.cache_key(),
            format!("multilingual-e5-small/{SMALL_REVISION}/float32")
        );
        assert_eq!(keys[0], format!("multilingual-e5-small/{SMALL_REVISION}/float32/ca456c06b3a9505ddfd9131408916dd79290368331e7d76bb621f1cba6bc8665/mean-e5-prefix-v1/384/512/single-text"));
        for variant in variants {
            let encoded = serde_json::to_string(&variant).unwrap();
            assert_eq!(
                serde_json::from_str::<ModelSelection>(&encoded).unwrap(),
                variant
            );
            for asset in variant.assets() {
                assert!(asset.bytes > 0);
                assert_eq!(asset.sha256.len(), 64);
                assert!(asset.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()));
                assert!(variant.asset_url(&asset).contains(&format!(
                    "/{}/resolve/{}/",
                    variant.model_name(),
                    variant.revision()
                )));
            }
        }
        let large = ModelSelection::LargeFloat32.assets();
        assert_eq!(large[5].path, "onnx/model.onnx_data");
        assert!(large[5].bytes > 2_000_000_000);
        assert_ne!(large[4].sha256, large[5].sha256);
    }
}
