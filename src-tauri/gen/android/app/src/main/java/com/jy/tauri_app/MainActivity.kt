package com.jy.tauri_app

import android.net.wifi.WifiManager
import android.os.Bundle
import android.util.Log
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  private var multicastLock: WifiManager.MulticastLock? = null

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    holdMulticastLock()
  }

  /**
   * Android 默认在 Wi-Fi 驱动层丢弃所有组播帧（mDNS 使用 224.0.0.251:5353）。
   * 不持有 MulticastLock 时，mdns-sd 一个 mDNS 包都收不到，设备扫描永远为空。
   * 必须在发现服务开始前持有锁，并保持到应用退出。
   */
  private fun holdMulticastLock() {
    try {
      val wifi = applicationContext.getSystemService(WIFI_SERVICE) as WifiManager
      multicastLock = wifi.createMulticastLock("PhraseApp_mDNS").apply {
        setReferenceCounted(false)
        acquire()
      }
      Log.i("MainActivity", "MulticastLock 已持有，mDNS 组播接收已启用")
    } catch (e: Exception) {
      Log.w("MainActivity", "获取 MulticastLock 失败: ${e.message}")
    }
  }

  override fun onDestroy() {
    multicastLock?.let { lock ->
      if (lock.isHeld) lock.release()
    }
    multicastLock = null
    super.onDestroy()
  }
}
