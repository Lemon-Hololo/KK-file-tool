import { ElMessageBox } from "element-plus";
import { normalizeInputPaths } from "../services/task";

export async function usePathNormalize(paths: string[]) {
  const result = await normalizeInputPaths(paths);

  if (result.warnings?.length) {
    await ElMessageBox.alert(result.warnings.join("<br/>"), "路径规范化提示", {
      dangerouslyUseHTMLString: true
    });
  }

  return result;
}
