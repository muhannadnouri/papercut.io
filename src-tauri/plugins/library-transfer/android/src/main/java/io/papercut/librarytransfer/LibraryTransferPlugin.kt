package io.papercut.librarytransfer

import android.app.Activity
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Channel
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin

@InvokeArg
class ObserveArgs { lateinit var onState: Channel }

@TauriPlugin
class LibraryTransferPlugin(private val activity: Activity) : Plugin(activity) {
    private var channel: Channel? = null
    private var stopped = false

    @Command
    fun observe(invoke: Invoke) {
        channel = invoke.parseArgs(ObserveArgs::class.java).onState
        channel?.sendObject(stopped)
        invoke.resolve()
    }

    // onPause also fires for permission prompts. Only stop when actually hidden.
    override fun onStop() {
        if (!activity.isChangingConfigurations) {
            stopped = true
            channel?.sendObject(true)
        }
    }

    override fun onResume() {
        stopped = false
        channel?.sendObject(false)
    }
}
