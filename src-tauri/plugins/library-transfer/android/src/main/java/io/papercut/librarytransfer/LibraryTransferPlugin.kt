package io.papercut.librarytransfer

import android.app.Activity
import android.os.Build
import android.os.SystemClock
import app.tauri.plugin.JSObject
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
    private var discovery: TransferDiscovery? = null
    private var latestDiscoveryId = 0L
    private var pendingDiscovery: DiscoveryArgs? = null
    private var cleanupDeadline = 0L

    @Command
    fun discoveryStart(invoke: Invoke) = activity.runOnUiThread {
        invoke.resolve(JSObject().put("state", startDiscovery(invoke.parseArgs(DiscoveryArgs::class.java))))
    }

    internal fun startDiscovery(args: DiscoveryArgs): String {
        if (Build.VERSION.SDK_INT < 34) return "unsupported"
        if (stopped || args.id <= 0 || args.id < latestDiscoveryId) return "unavailable"
        latestDiscoveryId = args.id
        pendingDiscovery = args
        cleanupDeadline = SystemClock.elapsedRealtime() + 5_000
        discovery?.stop()
        return if (discovery?.released == false) "starting" else beginDiscovery()
    }

    private fun beginDiscovery(): String {
        val args = pendingDiscovery ?: return "unavailable"
        pendingDiscovery = null
        return TransferDiscovery(activity, args, activity.mainExecutor).also { discovery = it }.start()
    }

    @Command
    fun discoveryPoll(invoke: Invoke) = activity.runOnUiThread {
        invoke.resolve(JSObject().put("events", pollDiscovery(invoke.parseArgs(DiscoveryId::class.java).id)))
    }

    internal fun pollDiscovery(id: Long): org.json.JSONArray {
        pendingDiscovery?.takeIf { it.id == id }?.let {
            val state = when {
                SystemClock.elapsedRealtime() >= cleanupDeadline -> { pendingDiscovery = null; "unavailable" }
                discovery?.released == false -> return org.json.JSONArray()
                else -> beginDiscovery()
            }
            if (state != "starting") return org.json.JSONArray().put(JSObject().put("kind", "error").put("state", state))
        }
        return if (discovery?.args?.id == id) discovery!!.poll() else org.json.JSONArray()
    }

    @Command
    fun discoveryStop(invoke: Invoke) = activity.runOnUiThread {
        stopDiscovery(invoke.parseArgs(DiscoveryId::class.java).id)
        invoke.resolve()
    }

    internal fun stopDiscovery(id: Long) {
        if (pendingDiscovery?.id == id) pendingDiscovery = null
        if (discovery?.args?.id == id) discovery?.stop()
    }

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
            pendingDiscovery = null
            discovery?.stop()
            channel?.sendObject(true)
        }
    }

    override fun onResume() {
        stopped = false
        channel?.sendObject(false)
    }
}
