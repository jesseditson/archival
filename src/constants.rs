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
pub const CLI_TOKEN_PUBLIC_KEY: &str = "-----BEGIN PUBLIC KEY-----
MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEA3HeKi5JVUwkD7jljXa8gF
Y0pxp+FzcbuHu5O3sIrY2nbGNr6zQ6zlxDWO7vIb1eUSzYif2WsWOprtnQ6nV+oQP
lyxDZLOt4Z4gl6ScJIpOfvRlo1V8HewehC8zCjv4AgoFu4dFPJyzi2UKAx74z8rjo
kCEfJ72jnz/xg4Qb6h0Flb4tPcFWCgm6PuUl6SZ49KCRiPZgllmJOtT9fh8CjQPT3
dG5tR4NTZVIm2oDgD/iNljy0e4cGk6vydBKIoKFt6jMzx8jnEvWqa2ULJ05n00hGV
TDmSx9zWcy8PETiYxKlIHcLgx1wkNpTeMibjf19bhNScuUecQpmVuY2uL3X6n5Q3N
sTr+GDmTAPDn9IHcQxLpjWDTEh/MIvheKQJeU3glBS7/ZjFUX1so83di/9XJc/JKY
R9V56St/Wjkuz/wpuRbnIpDWYriZ0QBl6AznzztOz07eRia5MWyvFp1OzBA5SpZcm
c3jxmhlvCQyKJPPU2zs9C6r0cszTj9/9J8+0ZBqM1V8wgemXTAvEJAwkVB7+Ye4Dy
Dq9EcLLCd92zFNZfexPKqohKxvDk+iDODhY/CvQOf7N2wq47FCrklimGeo/sVVzi8
yv5G+qPQmfmcH1D+K0dB77digpvScqWokiTCUh62xZpD6JdEVHtfcM1twaUzuuVwr
Q4+xXqcnS07cCAwEAAQ==
-----END PUBLIC KEY-----";
