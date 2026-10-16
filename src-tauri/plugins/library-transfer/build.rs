fn main() {
    tauri_plugin::Builder::new(&["observe"])
        .android_path("android")
        .ios_path("ios")
        .build();
}
