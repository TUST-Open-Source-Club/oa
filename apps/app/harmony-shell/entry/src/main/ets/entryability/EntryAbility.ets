import UIAbility from '@ohos.app.ability.UIAbility';
import window from '@ohos.window';
import hilog from '@ohos.hilog';

/** 应用入口：加载首页（服务器设置或 Web 门户）。 */
export default class EntryAbility extends UIAbility {
  onCreate(): void {
    hilog.info(0x0000, 'ClubOA', 'EntryAbility onCreate');
  }

  onWindowStageCreate(windowStage: window.WindowStage): void {
    windowStage.loadContent('pages/Index', (err) => {
      if (err.code) {
        hilog.error(0x0000, 'ClubOA', 'loadContent failed: %{public}s', JSON.stringify(err));
      }
    });
  }

  /** 深链：cluboa://...?server= 时预置服务器地址（页面读取 want 参数）。 */
  onNewWant(): void {
    hilog.info(0x0000, 'ClubOA', 'EntryAbility onNewWant');
  }
}
