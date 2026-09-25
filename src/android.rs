//! Runtime Android permission requests (JNI).
//!
//! Android 11+ has no runtime dialog for "All files access": the app must send
//! the user to `Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION`. Older
//! devices use the normal `requestPermissions` dialog for
//! `READ_EXTERNAL_STORAGE`.
//!
//! The raw `JavaVM`/activity pointers come from `android-activity` (via Bevy's
//! `ANDROID_APP`). We use the classic `jni` 0.21 API here; the pointers are
//! plain C pointers, so mixing versions is fine.
//!
//! SPDX-License-Identifier: MIT

#[cfg(target_os = "android")]
mod imp {
    use jni::objects::{JObject, JValue};
    use jni::sys::{jint, jobject};
    use jni::JavaVM;

    /// Arbitrary request code for the legacy permission dialog.
    const REQUEST_CODE: jint = 0x4152; // "AR"

    fn with_env<T>(
        f: impl FnOnce(&mut jni::JNIEnv, &JObject) -> jni::errors::Result<T>,
    ) -> Option<T> {
        let app = bevy::android::ANDROID_APP.get()?;
        // SAFETY: `vm_as_ptr`/`activity_as_ptr` return the live VM and activity
        // pointers owned by `android-activity` for as long as the app runs.
        let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr() as *mut jni::sys::JavaVM) }.ok()?;
        let mut env = vm.attach_current_thread().ok()?;
        let activity = unsafe { JObject::from_raw(app.activity_as_ptr() as jobject) };
        f(&mut env, &activity).ok()
    }

    fn sdk_int(env: &mut jni::JNIEnv) -> jni::errors::Result<jint> {
        env.get_static_field("android/os/Build$VERSION", "SDK_INT", "I")?
            .i()
    }

    /// Whether the app can read shared storage.
    pub fn granted() -> bool {
        with_env(|env, _activity| {
            env.call_static_method(
                "android/os/Environment",
                "isExternalStorageManager",
                "()Z",
                &[],
            )?
            .z()
        })
        .unwrap_or(false)
    }

    /// Ask for storage access. Returns true if an intent/dialog was launched.
    pub fn request() -> bool {
        with_env(|env, activity| {
            if sdk_int(env)? >= 30 {
                open_all_files_settings(env, activity)
            } else {
                request_legacy_permission(env, activity)
            }
        })
        .unwrap_or(false)
    }

    /// Send the user to the "All files access" settings page for this app.
    fn open_all_files_settings(
        env: &mut jni::JNIEnv,
        activity: &JObject,
    ) -> jni::errors::Result<bool> {
        // Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION)
        //   .setData(Uri.parse("package:<pkg>"))
        let action = env.new_string("android.settings.MANAGE_APP_ALL_FILES_ACCESS_PERMISSION")?;
        let intent = env.new_object(
            "android/content/Intent",
            "(Ljava/lang/String;)V",
            &[JValue::Object(action.as_ref())],
        )?;
        let uri_string = env.new_string(format!("package:{}", crate::data::ANDROID_PACKAGE))?;
        let uri = env.call_static_method(
            "android/net/Uri",
            "parse",
            "(Ljava/lang/String;)Landroid/net/Uri;",
            &[JValue::Object(uri_string.as_ref())],
        )?;
        let uri = uri.l()?;
        let _ = env.call_method(
            &intent,
            "setData",
            "(Landroid/net/Uri;)Landroid/content/Intent;",
            &[JValue::Object(&uri)],
        );

        let started = env
            .call_method(
                activity,
                "startActivity",
                "(Landroid/content/Intent;)V",
                &[JValue::Object(&intent)],
            )
            .is_ok();
        if started {
            return Ok(true);
        }

        // Some devices have no per-app page; fall back to the global list.
        let fallback_action = env.new_string("android.settings.MANAGE_ALL_FILES_ACCESS_PERMISSION")?;
        let fallback = env.new_object(
            "android/content/Intent",
            "(Ljava/lang/String;)V",
            &[JValue::Object(fallback_action.as_ref())],
        )?;
        Ok(env
            .call_method(
                activity,
                "startActivity",
                "(Landroid/content/Intent;)V",
                &[JValue::Object(&fallback)],
            )
            .is_ok())
    }

    /// Android 10 and older: request READ_EXTERNAL_STORAGE at runtime.
    fn request_legacy_permission(
        env: &mut jni::JNIEnv,
        activity: &JObject,
    ) -> jni::errors::Result<bool> {
        let permission = env.new_string("android.permission.READ_EXTERNAL_STORAGE")?;
        let array = env.new_object_array(1, "java/lang/String", JObject::null())?;
        env.set_object_array_element(&array, 0, &*permission)?;
        Ok(env
            .call_method(
                activity,
                "requestPermissions",
                "([Ljava/lang/String;I)V",
                &[JValue::Object(array.as_ref()), JValue::Int(REQUEST_CODE)],
            )
            .is_ok())
    }
}

#[cfg(target_os = "android")]
pub use imp::{granted, request};

/// On non-Android platforms there is nothing to request.
#[cfg(not(target_os = "android"))]
pub fn granted() -> bool {
    true
}

/// On non-Android platforms this is a no-op.
#[cfg(not(target_os = "android"))]
pub fn request() -> bool {
    false
}
