import type { Component } from "vue";

/** 工具会话控制器：关闭时由 toolSession 经 registry 调度。 */
export interface ToolSessionController {
  dispose: () => Promise<void>;
}

/** 编译期工具贡献点；系统 Settings 不进此 registry。 */
export interface ToolDefinition {
  id: string;
  labelKey: string;
  descriptionKey?: string;
  /** 图标标识，Launcher 内映射为 SVG */
  icon: string;
  order: number;
  stage: Component;
  /** 回桌面是否保活；false 时 deactivate 等同 dispose。默认 true */
  persistOnDeactivate?: boolean;
  /** 工具自有配置段；无则 Launcher「配置」禁用 */
  settingsSection?: Component;
  session?: ToolSessionController;
}
