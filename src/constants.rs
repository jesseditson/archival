pub const MIN_COMPAT_VERSION: &str = ">=0.8.0";
pub const MANIFEST_FILE_NAME: &str = "manifest.toml";
pub const OBJECT_DEFINITION_FILE_NAME: &str = "objects.toml";
pub const PAGES_DIR_NAME: &str = "pages";
pub const OBJECTS_DIR_NAME: &str = "objects";
pub const BUILD_DIR_NAME: &str = "dist";
pub const SCHEMAS_DIR_NAME: &str = "schemas";
pub const STATIC_DIR_NAME: &str = "public";
pub const LAYOUT_DIR_NAME: &str = "layout";
pub const NESTED_TYPES: [&str; 5] = ["meta", "upload", "video", "audio", "image"];
#[cfg(debug_assertions)]
pub const UPLOADS_URL: &str = "http://localhost:7777";
#[cfg(not(debug_assertions))]
pub const UPLOADS_URL: &str = "https://uploads.archival.dev";
#[cfg(debug_assertions)]
#[cfg(feature = "binary")]
pub const API_URL: &str = "http://localhost:8777";
#[cfg(not(debug_assertions))]
#[cfg(feature = "binary")]
pub const API_URL: &str = "https://api.archival.dev";
#[cfg(debug_assertions)]
#[cfg(feature = "binary")]
pub const AUTH_URL: &str = "http://localhost:8788/cli-login";
#[cfg(not(debug_assertions))]
#[cfg(feature = "binary")]
pub const AUTH_URL: &str = "https://archival.dev/cli-login";
#[cfg(debug_assertions)]
#[cfg(feature = "binary")]
pub const CLI_TOKEN_PUBLIC_KEY: &str = r#"-----BEGIN PUBLIC KEY-----
MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEA8QoZ+84kiRHUf2EexGWg
+T2Cmajg1K9NYT4v4mAxtBPnmdmyMUf6nOhdmweVfRMA74/hGZHxLLAWnq6xDpf7
b90I2ypmMv+0uiLZUGJGwYs6gpnk3PuFvHtsPYMnXviC6XtiP16G8LJ32bnv/ICB
nIFew9EFKmdFreh2OqWvyS/gD3C+kaG9rt0za2+MnY6g3JdRD2HKlcThI+bhvXFk
RLvkQdXFgjEa463y+nl2QGQ8my7GJVK0qdpmvld9gtVbpuiPjHcXWjL5qNvxHNFM
ObleQUIzB7A1ETyHNUFJHtjcnHu60jqMu8r2ZeyAVDLdd6u4J7saW34YkE6UyVsh
G9nkkVr2QDbEkcdm9Z/L1ptgtEok8V4g8v6/3KSPlV0sGh9lhGUZFLwKhv1PX+v4
gItvv8XzIcmNHv+MuhW42XKbUpUvA9phQEK5/idcxD15Qf/yzp365yOxzN3rbHy/
X8ZZ9Hds6exALtdidZCo8DvD3m7+SftoDLVBEQgZLFpcOehGVOOaKgZIWiyT5BXU
PnAQcpKroW5lGQWGR+/NBkPU4YBZ7bSrR909/Z4Zx/bSNAKLL3xFmB5L8kW36/tG
5hrnw2Z6nsZY0WVGCRKSEX4cDbMTKI4D0CBPtrF3Efp5Z1H51jhNS+8txmKcpiMU
LK6+Gd4/xCPzLSMLwxOFjE0CAwEAAQ==
-----END PUBLIC KEY-----"#;
#[cfg(not(debug_assertions))]
#[cfg(feature = "binary")]
pub const CLI_TOKEN_PUBLIC_KEY: &str = r#"-----BEGIN PUBLIC KEY-----
MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAyzG+whmGPnlX9VktuhhMh
c6sLyEuqRUYt2yh1/rfR3ZZXXH3IxP+zvzJdNYMBqiHtbRIOAzxzzTEberxzaqxm2
imfscqe989pfEZ8xZjrwGjdmIUyZY4HPIz1IWYUKYs7A+eZfqHWeCinRZjzW/rN1y
IWYZ56vPctCn8pG9ekzUrF+eU4KLjYfNGOAEoyPoXX4kiRgVYopMsUYwAJx5m8+OO
QVwvKHMhFb9i5LCx6S2YMUBHVC4pUDyIZEVTPqAaFrMpu6cl3qRB8hu+Ql7WhIsih
5e3NIVjW57EJY32Tjb16xh3AOfKvt9fNOCYnT03B309NMtX+ZZNOenVNQhNgaQ06T
KreNHdkiodzPWOz2UM+hi2P/t3kla4HuEap5P3u+svgkZ45BtLFKzfoOHoAaEdQSh
qxaXWBooJW3xpn6SBVYqfcMgAO7j0BH/Ft0j22QhQr6gc+kHbqE/ewIPbadjFc/+o
ziKnX6V3q2CaQTHNg75dFleNeyr+eS7zt9gQmTXV4NBw/zxL3sV44E80I7gjTEfc7
CyCjjMKkbNGICGdLv4p8FcH32MqaP0X9W7BLKW9Yd7MbIpW1ivSydDHNRXjlsiANP
0DEl3T/dPlUBqInjOoUfMB7fpK6Nc56KZfSda45BbPfzMuI4pOsWDchFfm7A8UGs3
/tzixpTvN8eECAwEAAQ==
-----END PUBLIC KEY-----"#;
