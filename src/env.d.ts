/// <reference types="vite/client" />

declare module "*.vue" {
  import type { DefineComponent } from "vue";
  const component: DefineComponent<object, object, unknown>;
  export default component;
}

declare module "*?worker" {
  const workerConstructor: {
    new (): Worker;
  };
  export default workerConstructor;
}

// Monaco 编辑贡献集（纯副作用导入：注册 suggest/format/右键/find 等 contrib，无导出）
declare module "monaco-editor/esm/vs/editor/editor.all.js";
