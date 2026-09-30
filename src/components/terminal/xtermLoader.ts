/** xterm 懒加载单例（对齐 monaco loadMonaco 模式：动态 import + 模块级缓存，
 *  @xterm 系列与 CSS 全部进独立异步 chunk，主包零增量）。 */

type XtermModule = typeof import("@xterm/xterm");
type FitAddonModule = typeof import("@xterm/addon-fit");
type WebLinksModule = typeof import("@xterm/addon-web-links");

export interface XtermBundle {
  Terminal: XtermModule["Terminal"];
  FitAddon: FitAddonModule["FitAddon"];
  WebLinksAddon: WebLinksModule["WebLinksAddon"];
}

let cached: Promise<XtermBundle> | null = null;

export function loadXterm(): Promise<XtermBundle> {
  if (!cached) {
    cached = (async () => {
      const [xterm, fit, webLinks, css] = await Promise.all([
        import("@xterm/xterm"),
        import("@xterm/addon-fit"),
        import("@xterm/addon-web-links"),
        // eslint-disable-next-line @typescript-eslint/no-unsafe-return -- Vite 把 css import 编译为样式注入
        import("@xterm/xterm/css/xterm.css") as Promise<unknown>,
      ]);
      void css;
      return {
        Terminal: xterm.Terminal,
        FitAddon: fit.FitAddon,
        WebLinksAddon: webLinks.WebLinksAddon,
      };
    })();
    cached.catch(() => {
      // 加载失败允许重试（网络/资源异常后不留死缓存）
      cached = null;
    });
  }
  return cached;
}
