//! File-origin classification from repository-relative path heuristics.
//!
//! See docs/language-extraction-rules.md §2.2. Used to down-rank generated and
//! vendor noise (FR-010) while keeping high-value local comments.

/// Classification of where a source file comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOrigin {
    LocalApplication,
    LocalDriver,
    LocalValidationModel,
    TestHarness,
    GeneratedStCube,
    GeneratedAiNetwork,
    VendorHalCmsis,
    ThirdPartyMiddleware,
    /// Default for ordinary local source with no stronger signal.
    Local,
}

impl FileOrigin {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            FileOrigin::LocalApplication => "local_application",
            FileOrigin::LocalDriver => "local_driver",
            FileOrigin::LocalValidationModel => "local_validation_model",
            FileOrigin::TestHarness => "test_harness",
            FileOrigin::GeneratedStCube => "generated_st_cube",
            FileOrigin::GeneratedAiNetwork => "generated_ai_network",
            FileOrigin::VendorHalCmsis => "vendor_hal_cmsis",
            FileOrigin::ThirdPartyMiddleware => "third_party_middleware",
            FileOrigin::Local => "local",
        }
    }

    /// Reconstruct from a serialized `as_str` label.
    #[must_use]
    pub fn from_label(label: &str) -> Self {
        match label {
            "local_application" => FileOrigin::LocalApplication,
            "local_driver" => FileOrigin::LocalDriver,
            "local_validation_model" => FileOrigin::LocalValidationModel,
            "test_harness" => FileOrigin::TestHarness,
            "generated_st_cube" => FileOrigin::GeneratedStCube,
            "generated_ai_network" => FileOrigin::GeneratedAiNetwork,
            "vendor_hal_cmsis" => FileOrigin::VendorHalCmsis,
            "third_party_middleware" => FileOrigin::ThirdPartyMiddleware,
            _ => FileOrigin::Local,
        }
    }

    /// Generated or vendor files are down-ranked / suppressed by default.
    #[must_use]
    pub const fn is_generated_or_vendor(self) -> bool {
        matches!(
            self,
            FileOrigin::GeneratedStCube
                | FileOrigin::GeneratedAiNetwork
                | FileOrigin::VendorHalCmsis
                | FileOrigin::ThirdPartyMiddleware
        )
    }
}

/// Classify a repository-relative path into a [`FileOrigin`].
///
/// More specific vendor/generated signals win over generic local ones.
#[must_use]
pub fn classify(path: &str) -> FileOrigin {
    let lower = path.replace('\\', "/").to_lowercase();
    let file = lower.rsplit('/').next().unwrap_or(&lower);

    if lower.contains("drivers/cmsis/") {
        return FileOrigin::VendorHalCmsis;
    }
    if lower.contains("middlewares/") {
        return FileOrigin::ThirdPartyMiddleware;
    }
    if file.starts_with("network.") || file.starts_with("network_data.") {
        return FileOrigin::GeneratedAiNetwork;
    }
    if file.starts_with("frame_validate.") {
        return FileOrigin::LocalValidationModel;
    }
    if is_test_path(&lower, file) {
        return FileOrigin::TestHarness;
    }
    if lower.contains("core/src/") || lower.contains("core/inc/") {
        return FileOrigin::LocalApplication;
    }
    if lower.contains("drivers/") {
        return FileOrigin::LocalDriver;
    }
    FileOrigin::Local
}

fn is_test_path(lower: &str, file: &str) -> bool {
    lower.contains("/tests/")
        || lower.contains("/test/")
        || file.starts_with("test_")
        || file.ends_with("_test.py")
        || file.ends_with("_test.c")
        || file.ends_with("_test.cpp")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vendor_and_generated_are_detected() {
        assert_eq!(classify("Drivers/CMSIS/foo.c"), FileOrigin::VendorHalCmsis);
        assert_eq!(
            classify("Middlewares/ThreadX/tx.c"),
            FileOrigin::ThirdPartyMiddleware
        );
        assert_eq!(
            classify("X-CUBE-AI/App/network.c"),
            FileOrigin::GeneratedAiNetwork
        );
        assert!(classify("Drivers/CMSIS/foo.c").is_generated_or_vendor());
    }

    #[test]
    fn local_and_validation_and_test() {
        assert_eq!(
            classify("src/frame_validate.c"),
            FileOrigin::LocalValidationModel
        );
        assert_eq!(classify("Core/Src/main.c"), FileOrigin::LocalApplication);
        assert_eq!(classify("tests/test_x.py"), FileOrigin::TestHarness);
        assert_eq!(classify("pkg/util.py"), FileOrigin::Local);
    }
}
