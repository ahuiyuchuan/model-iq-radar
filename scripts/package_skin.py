"""把目录式皮肤打成可导入的自包含 JSON；作者 ahui。

依赖：Python 3、Pillow。示例：
python scripts/package_skin.py public/skins/blue-whale-maid --id my-pet --output C:/tmp/my-pet.json
读取 skin.json 与 PNG；不联网、不改源文件，输出已存在时拒绝覆盖。
成功输出路径及大小，异常退出非零；运行时仍会执行前端与 Rust 双重校验。
"""
import argparse
import base64
import json
import re
from pathlib import Path

from PIL import Image


def main():
    """校验引用路径和实际尺寸后写入新文件；内置 ID 必须显式改为自定义 ID。"""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--id", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]{0,63}", args.id) or args.id in ("classic", "blue-whale-maid"):
        parser.error("请使用新的自定义 ID，不能覆盖内置皮肤。")
    source = args.directory / "skin.json"
    manifest = json.loads(source.read_text(encoding="utf-8"))
    manifest["id"] = args.id
    assets = {}
    if manifest["schemaVersion"] != 1:
        raise ValueError("仅支持版本 1 的 PNG 图集皮肤。")
    if "decorations" in manifest and manifest["decorations"] not in ("none", "ocean", "stars"):
        raise ValueError("面板装饰仅支持 none、ocean 或 stars。")
    # 与宿主共享可选动作集合；新增动作不要求旧包补齐，未知名称应在离线制作阶段失败。
    actions = manifest.get("actions", {})
    allowed = {"idle", "petting", "dragging", "working", "success", "seated", "wave", "stretch", "landing", "bow", "failed", "error"}
    if "idle" not in actions or set(actions) - allowed:
        raise ValueError("皮肤必须提供待机，且不能包含未知动作。")
    clips = list(manifest["actions"].values())
    # left是历史通用源图字段，宿主固定适配三边；离线包不提供可执行变换配置。
    if "docking" in manifest:
        docking = manifest["docking"]
        if not isinstance(docking, dict) or set(docking) != {"left"}:
            raise ValueError("探头须使用 docking.left 声明通用静态源图。")
        left = docking["left"]
        if (not isinstance(left, dict) or left.get("frames") != 1
                or left.get("columns") != 1 or "blink" in left
                or type(left.get("frameSize")) is not int
                or not 32 <= left["frameSize"] <= 512):
            raise ValueError("探头必须为单帧方形 PNG。")
        clips.append(left)
    for clip in clips:
        for kind in ("atlas", "poster"):
            name = clip[kind]
            if not re.fullmatch(r"[a-zA-Z0-9_-]+\.png", name):
                raise ValueError("资源只允许目录内的 PNG 文件名。")
            path = args.directory / name
            size = clip["frameSize"]
            expected = (size * clip["columns"], size * ((clip["frames"] + clip["columns"] - 1) // clip["columns"])) if kind == "atlas" else (size, size)
            with Image.open(path) as image:
                if image.format != "PNG" or image.size != expected:
                    raise ValueError(f"资源尺寸不一致：{name}，预期 {expected}。")
                # 头部和尺寸正确仍可能截断；打包前完整解码，不将坏PNG交给用户安装。
                image.load()
            assets[name] = "data:image/png;base64," + base64.b64encode(path.read_bytes()).decode("ascii")
    data = json.dumps({"manifest": manifest, "assets": assets}, ensure_ascii=False, separators=(",", ":")) + "\n"
    if len(data.encode("utf-8")) > 48 * 1024**2:
        raise ValueError("皮肤包超过 48 MiB。")
    with args.output.open("x", encoding="utf-8", newline="\n") as output:
        output.write(data)
    print(f"Created {args.output} ({len(data.encode('utf-8'))} bytes)")


if __name__ == "__main__":
    main()
