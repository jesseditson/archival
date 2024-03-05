pub const MANIFEST_FILE_NAME: &str = "manifest.toml";
pub const OBJECT_DEFINITION_FILE_NAME: &str = "objects.toml";
pub const PAGES_DIR_NAME: &str = "pages";
pub const OBJECTS_DIR_NAME: &str = "objects";
pub const BUILD_DIR_NAME: &str = "dist";
pub const STATIC_DIR_NAME: &str = "public";
pub const LAYOUT_DIR_NAME: &str = "layout";
pub const CDN_URL: &str = "https://cdn.archival.dev";
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
pub const CLI_TOKEN_PUBLIC_KEY: &str = "-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAtsQsUV8QpqrygsY+2+JC
Q6Fw8/omM71IM2N/R8pPbzbgOl0p78MZGsgPOQ2HSznjD0FPzsH8oO2B5Uftws04
LHb2HJAYlz25+lN5cqfHAfa3fgmC38FfwBkn7l582UtPWZ/wcBOnyCgb3yLcvJrX
yrt8QxHJgvWO23ITrUVYszImbXQ67YGS0YhMrbixRzmo2tpm3JcIBtnHrEUMsT0N
fFdfsZhTT8YbxBvA8FdODgEwx7u/vf3J9qbi4+Kv8cvqyJuleIRSjVXPsIMnoejI
n04APPKIjpMyQdnWlby7rNyQtE4+CV+jcFjqJbE/Xilcvqxt6DirjFCvYeKYl1uH
LwIDAQAB
-----END PUBLIC KEY-----";
