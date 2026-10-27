package com.cluboahq.app

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.view.ViewGroup
import android.webkit.JavascriptInterface
import android.webkit.PermissionRequest
import android.webkit.ValueCallback
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.launch

/**
 * 主界面：未配置服务器时显示服务器设置页，配置后加载 Web 门户（PWA）。
 * 见需求 13.1.1：服务器地址不硬编码，可随时切换。
 */
class MainActivity : AppCompatActivity() {
    private var webView: WebView? = null
    private var filePathCallback: ValueCallback<Array<Uri>>? = null
    private val fileChooserCode = 1001

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // 深链携带 ?server= 时预置服务器地址
        intent?.data?.getQueryParameter("server")?.let { candidate ->
            ServerConfig.set(this, candidate)
        }
        if (ServerConfig.get(this) == null) showSetup() else showWeb()
    }

    /** 服务器设置页（首次启动/切换服务器）。 */
    private fun showSetup() {
        val padding = (resources.displayMetrics.density * 24).toInt()
        val layout = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(padding, padding * 2, padding, padding)
        }
        layout.addView(TextView(this).apply {
            text = getString(R.string.server_setup_title)
            textSize = 22f
        })
        layout.addView(EditText(this).apply {
            hint = getString(R.string.server_setup_hint)
            setSingleLine()
        }, LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT
        ))
        val submit = Button(this).apply { text = getString(R.string.server_setup_save) }
        layout.addView(submit)
        val status = TextView(this).apply { text = "" }
        layout.addView(status)
        val input = layout.getChildAt(1) as EditText
        submit.setOnClickListener {
            val baseUrl = input.text.toString()
            status.text = getString(R.string.server_setup_checking)
            submit.isEnabled = false
            lifecycleScope.launch {
                val result = ServerConfig.validate(baseUrl)
                submit.isEnabled = true
                result
                    .onSuccess { issuer ->
                        ServerConfig.set(this@MainActivity, baseUrl)
                        Toast.makeText(this@MainActivity, "已连接：$issuer", Toast.LENGTH_SHORT).show()
                        showWeb()
                    }
                    .onFailure { error ->
                        status.text = "连接失败：${error.message ?: "未知错误"}"
                    }
            }
        }
        setContentView(layout)
    }

    /** 加载 Web 门户。 */
    @SuppressLint("SetJavaScriptEnabled")
    private fun showWeb() {
        val baseUrl = ServerConfig.get(this) ?: return showSetup()
        val web = WebView(this)
        webView = web
        with(web.settings) {
            javaScriptEnabled = true
            domStorageEnabled = true
            mediaPlaybackRequiresUserGesture = false
            mixedContentMode = WebSettings.MIXED_CONTENT_NEVER_ALLOW
            userAgentString = "$userAgentString ClubOA-Android/0.1.0"
        }
        web.addJavascriptInterface(object {
            /** Web 侧查询平台标识。 */
            @JavascriptInterface
            fun platform(): String = "android"

            /** Web 侧查询厂商（用于选择推送 SDK）。 */
            @JavascriptInterface
            fun vendor(): String = PushBridge().vendor()

            /** Web 侧查询厂商 token（SDK 接入前为空）。 */
            @JavascriptInterface
            fun pushToken(): String? = PushBridge().token()

            /** Web 侧请求切换服务器：清除配置并回到设置页（需求 AND-010）。 */
            @JavascriptInterface
            fun switchServer() {
                runOnUiThread {
                    ServerConfig.clear(this@MainActivity)
                    showSetup()
                }
            }
        }, "ClubOA")
        web.webViewClient = object : WebViewClient() {
            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                val url = request.url
                // 站外链接交给系统浏览器
                return if (url.host == Uri.parse(baseUrl).host) false else {
                    startActivity(Intent(Intent.ACTION_VIEW, url)); true
                }
            }
        }
        web.webChromeClient = object : WebChromeClient() {
            override fun onPermissionRequest(request: PermissionRequest) {
                // 会议/扫码所需的摄像头与麦克风
                request.grant(request.resources)
            }

            override fun onShowFileChooser(
                view: WebView,
                callback: ValueCallback<Array<Uri>>,
                params: FileChooserParams,
            ): Boolean {
                filePathCallback = callback
                startActivityForResult(params.createIntent(), fileChooserCode)
                return true
            }
        }
        setContentView(web)
        web.loadUrl("$baseUrl/")
    }

    /** 返回键优先用于网页内回退。 */
    @Deprecated("Deprecated in Java")
    override fun onBackPressed() {
        val web = webView
        if (web != null && web.canGoBack()) web.goBack() else super.onBackPressed()
    }

    override fun onDestroy() {
        webView?.destroy()
        webView = null
        super.onDestroy()
    }
}
