import router from '@ohos.router';
import { ServerConfig } from '../utils/ServerConfig';

/**
 * 服务器设置页（见需求 13.1.1）：首次启动校验 OIDC discovery 后保存并进入 Web 门户。
 */
@Entry
@Component
struct Index {
  @State baseUrl: string = '';
  @State checking: boolean = false;
  @State message: string = '';

  async aboutToAppear(): Promise<void> {
    const saved = await ServerConfig.get(getContext(this));
    if (saved) {
      router.replaceUrl({ url: 'pages/WebPage' });
    }
  }

  async connect(): Promise<void> {
    if (this.checking) return;
    this.checking = true;
    this.message = '正在校验服务器…';
    try {
      const issuer = await ServerConfig.validate(this.baseUrl);
      await ServerConfig.set(getContext(this), this.baseUrl);
      this.message = `已连接：${issuer}`;
      router.replaceUrl({ url: 'pages/WebPage' });
    } catch (error) {
      this.message = `连接失败：${(error as Error).message}`;
    } finally {
      this.checking = false;
    }
  }

  build() {
    Column({ space: 16 }) {
      Text('配置服务器')
        .fontSize(24)
        .fontWeight(FontWeight.Bold)
      TextInput({ placeholder: 'https://oa.example.com' })
        .type(InputType.URL)
        .onChange((value: string) => { this.baseUrl = value; })
      Button(this.checking ? '校验中…' : '连接')
        .enabled(!this.checking)
        .width('100%')
        .onClick(() => { this.connect(); })
      Text(this.message)
        .fontSize(12)
        .fontColor('#666666')
    }
    .padding(24)
    .width('100%')
    .height('100%')
    .justifyContent(FlexAlign.Center)
  }
}
