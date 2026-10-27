import router from '@ohos.router';
import web_webview from '@ohos.web.webview';
import { ServerConfig } from '../utils/ServerConfig';

/**
 * Web 门户容器：ArkWeb 加载 PWA，注入 ClubOA 桥（vendor/pushToken）。
 * HarmonyOS NEXT 推送统一走华为 Push Kit（见需求 13.4 HARM-003）。
 */
@Entry
@Component
struct WebPage {
  controller: web_webview.WebviewController = new web_webview.WebviewController();
  @State baseUrl: string = '';
  @State ready: boolean = false;

  async aboutToAppear(): Promise<void> {
    this.baseUrl = await ServerConfig.get(getContext(this));
    this.ready = true;
  }

  build() {
    Column() {
      if (this.ready && this.baseUrl.length > 0) {
        Web({ src: `${this.baseUrl}/`, controller: this.controller })
          .javaScriptAccess(true)
          .domStorageAccess(true)
          .mediaPlayAudio(true)
          .darkMode(WebDarkMode.Auto)
          .onPermissionRequest((event) => {
            if (event) {
              event.request.grant(event.request.getAccessibleResource());
            }
          })
          .javaScriptProxy({
            object: {
              platform: (): string => 'harmony',
              vendor: (): string => 'huawei',
              pushToken: (): string | null => null,
              switchServer: (): void => {
                ServerConfig.clear(getContext(this)).then(() => {
                  router.replaceUrl({ url: 'pages/Index' });
                });
              }
            },
            name: 'ClubOA',
            methodList: ['platform', 'vendor', 'pushToken', 'switchServer'],
            controller: this.controller
          })
      } else {
        LoadingProgress().width(48).height(48)
      }
    }
    .width('100%')
    .height('100%')
  }
}
