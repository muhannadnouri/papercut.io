package io.papercut.librarytransfer

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.os.Build
import app.tauri.annotation.InvokeArg
import org.json.JSONArray
import org.json.JSONObject
import java.net.NetworkInterface
import java.util.ArrayDeque
import java.util.concurrent.Executor

@InvokeArg
class DiscoveryArgs {
    var id: Long = 0
    var interfaceName: String = ""
    var interfaceIndex: Int = 0
    var addresses: List<String> = emptyList()
    var name: String = ""
    var port: Int = 0
    var mode: String = ""
    var browse: Boolean = false
}
@InvokeArg
class DiscoveryId { var id: Long = 0 }

// Constructed only on API 34+. All callbacks and commands run on the activity thread.
class TransferDiscovery(private val context: Context, val args: DiscoveryArgs, private val executor: Executor) {
    private val nsd = context.getSystemService(NsdManager::class.java)
    private val connectivity = context.getSystemService(ConnectivityManager::class.java)
    private lateinit var network: Network
    private val events = ArrayDeque<JSONObject>()
    private var closed = false
    private var browser: NsdManager.DiscoveryListener? = null
    private var registration: NsdManager.RegistrationListener? = null
    private val services = mutableMapOf<String, NsdManager.ServiceInfoCallback>()
    private val handles = mutableSetOf<Any>()
    val released: Boolean get() = closed && handles.isEmpty()

    fun start(): String {
        if (Build.VERSION.SDK_INT < 34) return "unsupported"
        if (args.id <= 0 || args.interfaceName.length > 128 || args.interfaceIndex <= 0 ||
            args.addresses.isEmpty() || args.addresses.size > 8 || args.mode !in listOf("push", "pull") ||
            (!args.browse && (args.port !in 1024..65535 || !label(args.name)))) return "unavailable"
        return try {
            if (NetworkInterface.getByName(args.interfaceName)?.index != args.interfaceIndex) return "unavailable"
            val matches = connectivity.allNetworks.filter { candidate ->
                connectivity.getLinkProperties(candidate)?.let { properties ->
                    properties.interfaceName == args.interfaceName
                } == true
            }
            if (matches.size != 1) return "unavailable"
            network = matches.single()
            if (args.browse) browse() else advertise()
            "starting"
        } catch (_: SecurityException) { stop(); "denied" }
        catch (_: Exception) { stop(); "unavailable" }
    }
    private fun label(name: String) = name.isNotEmpty() && name.toByteArray(Charsets.UTF_8).size <= 63 && name.none { it.isISOControl() }
    private fun error(state: String) {
        if (closed) return
        events.clear(); events.add(JSONObject().put("kind", "error").put("state", state)); stop()
    }
    private fun emit(event: JSONObject) {
        if (closed) return
        if (events.size == 128) { error("limited"); return }
        events.add(event)
    }
    private fun browse() {
        val listener = object : NsdManager.DiscoveryListener {
            override fun onDiscoveryStarted(type: String) { if (closed) stopBrowser(this) }
            override fun onDiscoveryStopped(type: String) { handles.remove(this) }
            override fun onStartDiscoveryFailed(type: String, code: Int) { handles.remove(this); error("unavailable") }
            override fun onStopDiscoveryFailed(type: String, code: Int) { error("unavailable") }
            override fun onServiceFound(info: NsdServiceInfo) {
                if (closed || info.network != network || info.serviceType.trimEnd('.') != TYPE || !label(info.serviceName)) return
                val name = info.serviceName
                if (services.containsKey(name)) return
                if (services.size == 64 || handles.size >= 65) { error("limited"); return }
                val callback = object : NsdManager.ServiceInfoCallback {
                    override fun onServiceInfoCallbackRegistrationFailed(code: Int) {
                        handles.remove(this)
                        if (!closed && services[name] === this) { services.remove(name); error("unavailable") }
                    }
                    override fun onServiceInfoCallbackUnregistered() { handles.remove(this) }
                    override fun onServiceLost() { if (services[name] === this) lost(name) }
                    override fun onServiceUpdated(value: NsdServiceInfo) {
                        if (closed || services[name] !== this || value.network != network || value.serviceName != name) return
                        if (value.hostAddresses.size > 8 || value.attributes.size != 3 ||
                            value.attributes.values.any { it == null } ||
                            value.attributes.entries.sumOf { it.key.toByteArray(Charsets.UTF_8).size + it.value.size + 2 } > 512) {
                            emit(JSONObject().put("kind", "lost").put("name", name).put("source", "android"))
                            return
                        }
                        val addresses = JSONArray()
                        value.hostAddresses.forEach { address -> address.hostAddress?.let { addresses.put(it.substringBefore('%')) } }
                        val txt = JSONArray()
                        value.attributes.forEach { (key, bytes) ->
                            val field = key.toByteArray(Charsets.UTF_8) + byteArrayOf('='.code.toByte()) + bytes
                            txt.put(JSONArray().also { array -> field.forEach { array.put(it.toInt() and 255) } })
                        }
                        emit(JSONObject().put("kind", "peer").put("name", name).put("source", "android")
                            .put("addresses", addresses).put("port", value.port).put("txt", txt))
                    }
                }
                services[name] = callback; handles.add(callback)
                try { nsd.registerServiceInfoCallback(info, executor, callback) }
                catch (_: SecurityException) { handles.remove(callback); error("denied") }
                catch (_: Exception) { handles.remove(callback); error("unavailable") }
            }
            override fun onServiceLost(info: NsdServiceInfo) { if (info.network == network) lost(info.serviceName) }
        }
        browser = listener; handles.add(listener)
        try { nsd.discoverServices(TYPE, NsdManager.PROTOCOL_DNS_SD, network, executor, listener) }
        catch (error: Exception) { handles.remove(listener); throw error }
    }
    private fun lost(name: String) {
        if (closed) return
        services.remove(name)?.let { callback ->
            try { nsd.unregisterServiceInfoCallback(callback) } catch (_: Exception) { error("unavailable") }
        }
        emit(JSONObject().put("kind", "lost").put("name", name).put("source", "android"))
    }
    private fun advertise() {
        val listener = object : NsdManager.RegistrationListener {
            override fun onServiceRegistered(info: NsdServiceInfo) {
                if (closed) { stopRegistration(this); return }
                emit(JSONObject().put("kind", "ready").put("name", info.serviceName))
            }
            override fun onServiceUnregistered(info: NsdServiceInfo) { handles.remove(this) }
            override fun onRegistrationFailed(info: NsdServiceInfo, code: Int) { handles.remove(this); error("unavailable") }
            override fun onUnregistrationFailed(info: NsdServiceInfo, code: Int) { error("unavailable") }
        }
        val info = NsdServiceInfo().apply {
            serviceName = args.name; serviceType = TYPE; port = args.port; setNetwork(this@TransferDiscovery.network)
            setAttribute("v", "1"); setAttribute("p", "PCLAN004"); setAttribute("m", args.mode)
        }
        registration = listener; handles.add(listener)
        try { nsd.registerService(info, NsdManager.PROTOCOL_DNS_SD, executor, listener) }
        catch (error: Exception) { handles.remove(listener); throw error }
    }
    fun poll(): JSONArray {
        if (!closed && (!::network.isInitialized || connectivity.getLinkProperties(network)?.interfaceName != args.interfaceName)) {
            emit(JSONObject().put("kind", "networkChanged")); stop()
        }
        return JSONArray().also { array -> while (events.isNotEmpty()) array.put(events.removeFirst()) }
    }
    private fun stopBrowser(listener: NsdManager.DiscoveryListener) { try { nsd.stopServiceDiscovery(listener) } catch (_: Exception) { /* Retain handle; another start stays unavailable. */ } }
    private fun stopRegistration(listener: NsdManager.RegistrationListener) { try { nsd.unregisterService(listener) } catch (_: Exception) { /* Await the registration callback or keep discovery unavailable. */ } }
    fun stop() {
        if (closed) return
        closed = true
        browser?.let { stopBrowser(it) }
        registration?.let { stopRegistration(it) }
        services.values.toList().forEach { try { nsd.unregisterServiceInfoCallback(it) } catch (_: Exception) { /* Keep pending handles bounded until process exit. */ } }
        services.clear()
    }
    companion object { private const val TYPE = "_papercut-xfer._tcp" }
}
