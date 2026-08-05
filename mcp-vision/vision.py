#!/usr/bin/env python3
"""智谱 GLM 视觉 API 封装（OpenAI 兼容接口）。

- 模型: glm-4.6v-flash（智谱视觉模型，性价比高）
- 端点: https://open.bigmodel.cn/api/paas/v4/chat/completions
- API Key: 环境变量 ZHIPU_API_KEY 或 ~/.config/novaenv-vision/config.json 的 api_key
"""

import base64
import io
import json
import os
import sys

try:
    from openai import OpenAI
except ImportError:
    OpenAI = None

try:
    from PIL import Image
except ImportError:
    Image = None

MODEL = os.environ.get("VISION_MODEL", "glm-4.6v-flash")
BASE_URL = "https://open.bigmodel.cn/api/paas/v4"
MAX_EDGE = 2048          # 压缩后最长边像素
MAX_BYTES = 1_500_000    # 压缩后体积上限（1.5MB）
DEFAULT_TIMEOUT = 120

IMAGE_TYPES = {
    ".png": "image/png",
    ".jpg": "image/jpeg",
    ".jpeg": "image/jpeg",
    ".webp": "image/webp",
    ".gif": "image/gif",
    ".bmp": "image/bmp",
}


def _load_key() -> str:
    key = os.environ.get("ZHIPU_API_KEY", "").strip()
    if key:
        return key
    cfg = os.path.expanduser("~/.config/novaenv-vision/config.json")
    try:
        with open(cfg, "r", encoding="utf-8") as f:
            key = json.load(f).get("api_key", "").strip()
    except (OSError, ValueError):
        pass
    if not key:
        raise RuntimeError(
            "未找到智谱 API Key：请设置环境变量 ZHIPU_API_KEY，"
            "或在 ~/.config/novaenv-vision/config.json 写入 {\"api_key\": \"sk-...\"}"
        )
    return key


def _compress(data: bytes, mime: str) -> tuple[bytes, str]:
    """压缩图片：限制最长边与体积，返回 (bytes, mime)。"""
    if Image is None:
        return data, mime
    if len(data) <= MAX_BYTES:
        return data, mime
    try:
        img = Image.open(io.BytesIO(data))
        if img.width > MAX_EDGE or img.height > MAX_EDGE:
            ratio = MAX_EDGE / max(img.width, img.height)
            img = img.resize((int(img.width * ratio), int(img.height * ratio)), Image.LANCZOS)
        if img.mode not in ("RGB", "L"):
            img = img.convert("RGB")
        buf = io.BytesIO()
        img.save(buf, "JPEG", quality=82)
        return buf.getvalue(), "image/jpeg"
    except Exception:
        return data, mime


def ask_glm_vision(image_bytes: bytes, mime: str, question: str) -> str:
    """调用 GLM 视觉模型，返回文字回答。"""
    if OpenAI is None:
        raise RuntimeError("缺少依赖 openai：请先执行 pip install -r mcp-vision/requirements.txt")
    data, mime = _compress(image_bytes, mime)
    b64 = base64.b64encode(data).decode()
    client = OpenAI(api_key=_load_key(), base_url=BASE_URL, timeout=DEFAULT_TIMEOUT)
    resp = client.chat.completions.create(
        model=MODEL,
        messages=[
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": question},
                    {"type": "image_url", "image_url": {"url": f"data:{mime};base64,{b64}"}},
                ],
            }
        ],
        max_tokens=1024,
    )
    return resp.choices[0].message.content or ""


def main() -> int:
    """命令行直测：python3 vision.py <image_path> [question]"""
    if len(sys.argv) < 2:
        print("用法: python3 vision.py <image_path> [question]", file=sys.stderr)
        return 2
    path = sys.argv[1]
    question = sys.argv[2] if len(sys.argv) > 2 else "请描述这张图片的内容"
    ext = os.path.splitext(path)[1].lower()
    mime = IMAGE_TYPES.get(ext, "image/png")
    with open(path, "rb") as f:
        data = f.read()
    print(ask_glm_vision(data, mime, question))
    return 0


if __name__ == "__main__":
    sys.exit(main())
