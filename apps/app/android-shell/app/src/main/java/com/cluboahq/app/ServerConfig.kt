package com.cluboahq.app

import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.URL

/**
 * 服务器地址配置（不硬编码）：
 * - 首次启动需输入 Base URL，校验 OIDC discovery 后可保存
 * - 地址持久化在 SharedPreferences，切换服务器时清除
 */
object ServerConfig {
    private const val PREFS = "club_oa"
    private const val KEY_BASE_URL = "base_url"

    /** 读取已配置的服务器地址（未配置返回 null）。 */
    fun get(context: Context): String? =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getString(KEY_BASE_URL, null)

    /** 保存服务器地址（已归一化）。 */
    fun set(context: Context, baseUrl: String) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit().putString(KEY_BASE_URL, normalize(baseUrl)).apply()
    }

    /** 清除服务器配置（切换服务器前调用）。 */
    fun clear(context: Context) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().clear().apply()
    }

    /** 归一化：去尾部斜杠。 */
    fun normalize(baseUrl: String): String = baseUrl.trim().trimEnd('/')

    /**
     * 校验服务器：请求 `/.well-known/openid-configuration` 并返回 issuer。
     * 仅接受 HTTPS（本地联调允许 10.0.2.2/localhost 明文）。
     */
    suspend fun validate(baseUrl: String): Result<String> = withContext(Dispatchers.IO) {
        val normalized = normalize(baseUrl)
        if (!normalized.startsWith("https://") &&
            !normalized.startsWith("http://10.0.2.2") &&
            !normalized.startsWith("http://localhost")
        ) {
            return@withContext Result.failure(IllegalArgumentException("仅支持 HTTPS 服务器地址"))
        }
        try {
            val connection = URL("$normalized/.well-known/openid-configuration")
                .openConnection() as HttpURLConnection
            connection.connectTimeout = 8000
            connection.readTimeout = 8000
            connection.requestMethod = "GET"
            val body = connection.inputStream.bufferedReader().use { it.readText() }
            val issuer = JSONObject(body).optString("issuer")
            if (issuer.isNullOrBlank()) {
                Result.failure(IllegalStateException("服务器未返回合法的 OIDC issuer"))
            } else {
                Result.success(issuer)
            }
        } catch (error: Exception) {
            Result.failure(error)
        }
    }
}
