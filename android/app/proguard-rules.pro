# Horizon Android ProGuard Rules
# Keep all JNI methods called from native code
-keepclassmembers class net.bycesardev.horizon.MainActivity {
    native <methods>;
    public static native *;
}

# Keep the MainActivity itself
-keep class net.bycesardev.horizon.MainActivity { *; }

# Suppress notes about native methods
-dontnote net.bycesardev.horizon.**
