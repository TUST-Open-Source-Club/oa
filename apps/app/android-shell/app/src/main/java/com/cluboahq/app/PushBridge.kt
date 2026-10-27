package com.cluboahq.app

import android.os.Build
import android.util.Log

/**
 * 推送桥：Web 页面通过 `window.ClubOA` 调用。
 * - vendor 依据设备厂商识别（华为/荣耀/魅族/OPPO/vivo/小米，其余为 fcm）
 * - token 在各厂商 SDK 接入后回填；Web 侧调用 registerPushToken 上报
 */
class PushBridge {
    /** 识别设备厂商（与需求 13.7 的 vendor 列表对齐）。 */
    fun vendor(): String {
        val manufacturer = Build.MANUFACTURER.lowercase()
        return when {
            manufacturer.contains("huawei") -> "huawei"
            manufacturer.contains("honor") -> "honor"
            manufacturer.contains("xiaomi") || manufacturer.contains("redmi") -> "xiaomi"
            manufacturer.contains("oppo") || manufacturer.contains("oneplus") -> "oppo"
            manufacturer.contains("vivo") -> "vivo"
            manufacturer.contains("meizu") -> "meizu"
            else -> "fcm"
        }
    }

    /** 当前厂商 token（SDK 接入前返回 null，由前端跳过上报）。 */
    fun token(): String? {
        Log.i("PushBridge", "vendor=${vendor()}，厂商 SDK 未接入，token 为空")
        return null
    }
}
