import { useScriptRun } from "@/tools/scripts/useScriptRun";
import { useScripts } from "@/tools/scripts/useScripts";

/** 脚本库会话：关闭时清空列表/编辑态并取消运行。 */
export const scriptsSession = {
  async dispose(): Promise<void> {
    await useScriptRun().resetSession();
    await useScripts().resetSession();
  },
};
