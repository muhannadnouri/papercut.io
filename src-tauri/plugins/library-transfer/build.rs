fn main() {
    tauri_plugin::Builder::new(&[
        "observe",
        "discoveryStart",
        "discoveryPoll",
        "discoveryStop",
    ])
    .android_path("android")
    .ios_path("ios")
    .build();
}
