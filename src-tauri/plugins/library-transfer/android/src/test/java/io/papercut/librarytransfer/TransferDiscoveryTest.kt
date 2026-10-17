package io.papercut.librarytransfer

import android.app.Activity
import android.net.ConnectivityManager
import android.net.LinkProperties
import android.net.Network
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.Implementation
import org.robolectric.annotation.Implements
import org.robolectric.shadow.api.Shadow
import org.robolectric.shadows.ShadowNetwork
import org.robolectric.shadows.ShadowSystemClock
import java.net.InetAddress
import java.net.NetworkInterface
import java.time.Duration
import java.util.concurrent.Executor

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34], manifest = Config.NONE, shadows = [TestNsdManager::class, TestConnectivity::class])
class TransferDiscoveryTest {
    private lateinit var activity: Activity
    private lateinit var nsd: TestNsdManager
    private lateinit var connectivity: TestConnectivity
    private lateinit var selected: Network
    private lateinit var deviceInterface: NetworkInterface

    @Before fun setup() {
        activity = Robolectric.buildActivity(Activity::class.java).setup().get()
        nsd = Shadow.extract(activity.getSystemService(NsdManager::class.java))
        connectivity = Shadow.extract(activity.getSystemService(ConnectivityManager::class.java))
        deviceInterface = NetworkInterface.getNetworkInterfaces().toList().first()
        selected = ShadowNetwork.newInstance(42)
        connectivity.links[selected] = LinkProperties().apply { interfaceName = deviceInterface.name }
        connectivity.links[ShadowNetwork.newInstance(43)] = LinkProperties().apply { interfaceName = "other-interface" }
    }

    private fun args(id: Long = 1, browse: Boolean = true) = DiscoveryArgs().also {
        it.id = id; it.interfaceName = deviceInterface.name; it.interfaceIndex = deviceInterface.index
        it.addresses = listOf("192.168.1.2"); it.name = "Papercut-test"; it.port = 53318
        it.mode = "push"; it.browse = browse
    }
    private fun service() = NsdServiceInfo().apply {
        serviceName = "Papercut-peer"; serviceType = "_papercut-xfer._tcp"; port = 53318
        network = selected; hostAddresses = listOf(InetAddress.getByName("192.168.1.3"))
        setAttribute("v", "1"); setAttribute("p", "PCLAN004"); setAttribute("m", "push")
    }
    private fun browser() = TransferDiscovery(activity, args(), activity.mainExecutor).also {
        assertEquals("starting", it.start())
        nsd.browsers.last().listener.onServiceFound(service())
    }

    @Test fun publicationAndBrowsingUseOnlyTheSelectedNetwork() {
        val publication = TransferDiscovery(activity, args(browse = false), activity.mainExecutor)
        assertEquals("starting", publication.start())
        val registered = nsd.publications.single()
        assertEquals(selected, registered.info.network)
        assertEquals(53318, registered.info.port)
        assertEquals(setOf("v", "p", "m"), registered.info.attributes.keys)
        registered.listener.onServiceRegistered(registered.info)
        assertEquals("ready", publication.poll().getJSONObject(0).getString("kind"))
        publication.stop()
        browser()
        assertEquals(selected, nsd.browsers.single().network)
    }

    @Test fun nullTxtUpdateRemovesThePriorRowWithoutCrashingAndValidUpdatesRecover() {
        val discovery = browser()
        val callback = nsd.callbacks.single()
        callback.onServiceUpdated(service())
        assertEquals("peer", discovery.poll().getJSONObject(0).getString("kind"))
        for (key in listOf("v", "p", "m")) {
            callback.onServiceUpdated(service().apply { setAttribute(key, null as String?) })
            val lost = discovery.poll().getJSONObject(0)
            assertEquals("lost", lost.getString("kind"))
            assertEquals("Papercut-peer", lost.getString("name"))
        }
        callback.onServiceUpdated(service())
        assertEquals("peer", discovery.poll().getJSONObject(0).getString("kind"))
        assertTrue(nsd.stoppedCallbacks.isEmpty())
    }

    @Test fun replacedResolversCannotUpdateLoseOrFailTheCurrentRow() {
        val discovery = browser()
        val old = nsd.callbacks.single()
        nsd.browsers.single().listener.onServiceLost(service())
        assertTrue(nsd.stoppedCallbacks.contains(old))
        discovery.poll()
        nsd.browsers.single().listener.onServiceFound(service())
        val current = nsd.callbacks.last()
        current.onServiceUpdated(service()); discovery.poll()
        old.onServiceLost()
        old.onServiceUpdated(service().apply { port = 54321 })
        old.onServiceInfoCallbackRegistrationFailed(0)
        old.onServiceInfoCallbackUnregistered()
        assertEquals(0, discovery.poll().length())
        assertFalse(nsd.stoppedCallbacks.contains(current))
        current.onServiceUpdated(service())
        assertEquals(53318, discovery.poll().getJSONObject(0).getInt("port"))
        discovery.stop()
        current.onServiceLost(); current.onServiceUpdated(service()); current.onServiceInfoCallbackRegistrationFailed(0)
        assertEquals(0, discovery.poll().length())
    }

    @Test fun refreshWaitsForEveryHandleAndOnlyStartsTheLatestRequest() {
        val plugin = LibraryTransferPlugin(activity)
        assertEquals("starting", plugin.startDiscovery(args()))
        val first = nsd.browsers.single().listener
        first.onServiceFound(service())
        val callback = nsd.callbacks.single()
        assertEquals("starting", plugin.startDiscovery(args(2)))
        assertEquals("starting", plugin.startDiscovery(args(3)))
        plugin.stopDiscovery(1); plugin.stopDiscovery(2)
        assertEquals(0, plugin.pollDiscovery(2).length())
        first.onDiscoveryStopped("_papercut-xfer._tcp")
        assertEquals(0, plugin.pollDiscovery(3).length())
        assertEquals(1, nsd.browsers.size)
        callback.onServiceInfoCallbackUnregistered()
        plugin.pollDiscovery(2)
        assertEquals(1, nsd.browsers.size)
        plugin.pollDiscovery(3)
        assertEquals(2, nsd.browsers.size)
        nsd.browsers.last().listener.onServiceFound(service())
        nsd.callbacks.last().onServiceUpdated(service())
        assertEquals("peer", plugin.pollDiscovery(3).getJSONObject(0).getString("kind"))
        assertEquals("unavailable", plugin.startDiscovery(args(2)))
    }

    @Test fun cancellingPendingRefreshCannotBeUndoneByLateCleanup() {
        val plugin = LibraryTransferPlugin(activity)
        plugin.startDiscovery(args())
        plugin.startDiscovery(args(2)); plugin.stopDiscovery(2)
        nsd.browsers.single().listener.onDiscoveryStopped("_papercut-xfer._tcp")
        assertEquals(0, plugin.pollDiscovery(2).length())
        assertEquals(1, nsd.browsers.size)
        assertEquals("starting", plugin.startDiscovery(args(3)))
        assertEquals(2, nsd.browsers.size)
    }

    @Test fun backgroundingDiscardsPendingRefreshAndResumeRequiresAnExplicitRequest() {
        val plugin = LibraryTransferPlugin(activity)
        plugin.startDiscovery(args()); plugin.startDiscovery(args(2)); plugin.onStop()
        nsd.browsers.single().listener.onDiscoveryStopped("_papercut-xfer._tcp")
        plugin.pollDiscovery(2)
        assertEquals(1, nsd.browsers.size)
        assertEquals("unavailable", plugin.startDiscovery(args(3)))
        plugin.onResume(); plugin.pollDiscovery(2)
        assertEquals(1, nsd.browsers.size)
        assertEquals("starting", plugin.startDiscovery(args(4)))
        assertEquals(2, nsd.browsers.size)
    }

    @Test fun cleanupTimeoutFailsOnceAndLateAcknowledgementDoesNotStartTheTimedOutRequest() {
        val plugin = LibraryTransferPlugin(activity)
        plugin.startDiscovery(args()); plugin.startDiscovery(args(2))
        ShadowSystemClock.advanceBy(Duration.ofMillis(4_999))
        assertEquals(0, plugin.pollDiscovery(2).length())
        ShadowSystemClock.advanceBy(Duration.ofMillis(1))
        val error = plugin.pollDiscovery(2).getJSONObject(0)
        assertEquals("error", error.getString("kind")); assertEquals("unavailable", error.getString("state"))
        nsd.browsers.single().listener.onDiscoveryStopped("_papercut-xfer._tcp")
        assertEquals(0, plugin.pollDiscovery(2).length()); assertEquals(1, nsd.browsers.size)
        assertEquals("starting", plugin.startDiscovery(args(3)))
    }

    @Test fun pendingRefreshRevalidatesTheSelectedNetworkBeforeStarting() {
        val plugin = LibraryTransferPlugin(activity)
        plugin.startDiscovery(args()); plugin.startDiscovery(args(2))
        connectivity.links.remove(selected)
        nsd.browsers.single().listener.onDiscoveryStopped("_papercut-xfer._tcp")
        assertEquals("unavailable", plugin.pollDiscovery(2).getJSONObject(0).getString("state"))
        assertEquals(1, nsd.browsers.size)
    }

    @Test fun publicationCanWithdrawAndRepublishTheSameGenerationAfterDelayedCleanup() {
        val plugin = LibraryTransferPlugin(activity)
        plugin.startDiscovery(args(browse = false))
        val first = nsd.publications.single()
        first.listener.onServiceRegistered(first.info); plugin.pollDiscovery(1)
        plugin.stopDiscovery(1)
        assertEquals("starting", plugin.startDiscovery(args(browse = false)))
        assertEquals(0, plugin.pollDiscovery(1).length())
        first.listener.onServiceRegistered(first.info)
        first.listener.onServiceUnregistered(first.info)
        plugin.pollDiscovery(1)
        assertEquals(2, nsd.publications.size)
        val next = nsd.publications.last()
        assertEquals(first.info.serviceName, next.info.serviceName)
        next.listener.onServiceRegistered(next.info)
        assertEquals("ready", plugin.pollDiscovery(1).getJSONObject(0).getString("kind"))
    }
}

// Capture real API arguments; callbacks are deliberately driven separately to expose ordering races.
@Implements(NsdManager::class)
class TestNsdManager {
    data class Browse(val network: Network, val listener: NsdManager.DiscoveryListener)
    data class Publication(val info: NsdServiceInfo, val listener: NsdManager.RegistrationListener)
    val browsers = mutableListOf<Browse>()
    val publications = mutableListOf<Publication>()
    val callbacks = mutableListOf<NsdManager.ServiceInfoCallback>()
    val stoppedCallbacks = mutableSetOf<NsdManager.ServiceInfoCallback>()
    @Implementation fun discoverServices(type: String, protocol: Int, network: Network, executor: Executor, listener: NsdManager.DiscoveryListener) {
        assertEquals("_papercut-xfer._tcp", type); assertEquals(NsdManager.PROTOCOL_DNS_SD, protocol)
        browsers.add(Browse(network, listener))
    }
    @Implementation fun registerService(info: NsdServiceInfo, protocol: Int, executor: Executor, listener: NsdManager.RegistrationListener) {
        publications.add(Publication(info, listener))
    }
    @Implementation fun registerServiceInfoCallback(info: NsdServiceInfo, executor: Executor, callback: NsdManager.ServiceInfoCallback) { callbacks.add(callback) }
    @Implementation fun unregisterServiceInfoCallback(callback: NsdManager.ServiceInfoCallback) { stoppedCallbacks.add(callback) }
    @Implementation fun stopServiceDiscovery(listener: NsdManager.DiscoveryListener) {}
    @Implementation fun unregisterService(listener: NsdManager.RegistrationListener) {}
}

@Implements(ConnectivityManager::class)
class TestConnectivity {
    val links = mutableMapOf<Network, LinkProperties>()
    @Implementation fun getAllNetworks(): Array<Network> = links.keys.toTypedArray()
    @Implementation fun getLinkProperties(network: Network): LinkProperties? = links[network]
}
