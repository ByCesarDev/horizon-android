package net.bycesardev.horizon

import android.os.Bundle
import android.util.Log
import android.view.MotionEvent
import android.view.Surface
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.View
import android.view.WindowManager
import androidx.appcompat.app.AppCompatActivity
import java.io.File

class MainActivity : AppCompatActivity(), SurfaceHolder.Callback {

    private lateinit var surfaceView: SurfaceView

    companion object {
        private const val TAG = "HorizonActivity"

        init {
            try {
                System.loadLibrary("horizon_android")
                Log.i(TAG, "Successfully loaded libhorizon_android.so")
            } catch (e: UnsatisfiedLinkError) {
                Log.e(TAG, "Failed to load libhorizon_android.so: ${e.message}")
            }
        }

        // Native JNI functions implemented in crates/horizon-android
        @JvmStatic
        external fun nativeSetAssetManager(assetManager: Any)

        @JvmStatic
        external fun nativeInit(internalDataDir: String, resourcesDir: String): Boolean

        @JvmStatic
        external fun nativeSurfaceCreated(surface: Surface)

        @JvmStatic
        external fun nativeSurfaceChanged(width: Int, height: Int)

        @JvmStatic
        external fun nativeSurfaceDestroyed()

        @JvmStatic
        external fun nativeOnTouch(pointerId: Int, action: Int, x: Float, y: Float)

        @JvmStatic
        external fun nativeOnPause()

        @JvmStatic
        external fun nativeOnResume()

        @JvmStatic
        external fun nativeOnDestroy()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Keep screen on during gameplay
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        hideSystemUI()

        // Prepare paths for Horizon
        val internalDir = File(filesDir, "horizon").apply { mkdirs() }
        val resourcesDir = File(getExternalFilesDir(null) ?: filesDir, "resources").apply { mkdirs() }

        Log.i(TAG, "Internal data directory: ${internalDir.absolutePath}")
        Log.i(TAG, "Resources directory: ${resourcesDir.absolutePath}")

        nativeSetAssetManager(assets)
        val success = nativeInit(internalDir.absolutePath, resourcesDir.absolutePath)
        Log.i(TAG, "nativeInit result: $success")

        surfaceView = SurfaceView(this).apply {
            holder.addCallback(this@MainActivity)
        }
        setContentView(surfaceView)
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) {
            hideSystemUI()
        }
    }

    private fun hideSystemUI() {
        @Suppress("DEPRECATION")
        window.decorView.systemUiVisibility = (
            View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                or View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                or View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                or View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                or View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                or View.SYSTEM_UI_FLAG_FULLSCREEN
            )
    }

    // --- SurfaceHolder.Callback ---

    override fun surfaceCreated(holder: SurfaceHolder) {
        Log.i(TAG, "surfaceCreated: surface=${holder.surface}")
        nativeSurfaceCreated(holder.surface)
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        Log.i(TAG, "surfaceChanged: width=$width, height=$height, format=$format")
        nativeSurfaceChanged(width, height)
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        Log.i(TAG, "surfaceDestroyed")
        nativeSurfaceDestroyed()
    }

    // --- Touch Event Forwarding ---

    override fun onTouchEvent(event: MotionEvent): Boolean {
        val actionMasked = event.actionMasked
        val actionIndex = event.actionIndex

        when (actionMasked) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN -> {
                val pid = event.getPointerId(actionIndex)
                nativeOnTouch(pid, 0, event.getX(actionIndex), event.getY(actionIndex))
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP -> {
                val pid = event.getPointerId(actionIndex)
                nativeOnTouch(pid, 1, event.getX(actionIndex), event.getY(actionIndex))
            }
            MotionEvent.ACTION_MOVE -> {
                for (i in 0 until event.pointerCount) {
                    val pid = event.getPointerId(i)
                    nativeOnTouch(pid, 2, event.getX(i), event.getY(i))
                }
            }
            MotionEvent.ACTION_CANCEL -> {
                for (i in 0 until event.pointerCount) {
                    val pid = event.getPointerId(i)
                    nativeOnTouch(pid, 3, event.getX(i), event.getY(i))
                }
            }
        }
        return true
    }

    // --- Activity Lifecycle ---

    override fun onResume() {
        super.onResume()
        nativeOnResume()
    }

    override fun onPause() {
        super.onPause()
        nativeOnPause()
    }

    override fun onDestroy() {
        super.onDestroy()
        nativeOnDestroy()
    }
}
