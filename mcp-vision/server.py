#!/usr/bin/env python3
"""NovaEnv Vision MCP Server

让无视觉能力的模型（如 DeepSeek）通过 MCP 工具「看图」：
Reasonix / Cursor 等编辑器把图片传给本服务，本服务调用智谱 GLM-4.6V-Flash
多模态 API 返回文字分析，模型据此继续回答用户。

启动（stdio，供 MCP 客户端拉起）:
    python3 server.py

接入 Reasonix（项目 .mcp.json 或全局 MCP 配置）:
    {
      "mcpServers": {
        "vision": {
          "command": "python3",
          "args": ["/Users/macos/programs/1024/NovaHub/NovaEnv/mcp-vision/server.py"],
          "env": { "ZHIPU_API_KEY": "sk-xxxxxxxx" }
        }
      }
    }

Tools:
    analyze_image(image, question)  通用看图问答
    ocr_image(image)                提取图片文字
    describe_image(image)           一句话概括 + 关键细节

image 参数: 本地文件绝对路径，或 data:image/png;base64,.... 的 data URL。
"""

import base64
import os

from fastmcp import FastMCP

from vision import IMAGE_TYPES, ask_glm_vision

mcp = FastMCP("novaenv-vision")


def _load_image(source: str) -> tuple[bytes, str]:
    """source 为本地路径或 data URL，返回 (bytes, mime)。"""
    if source.startswith("data:"):
        header, _, b64 = source.partition(",")
        mime = header[5:].split(";")[0]
        return base64.b64decode(b64), mime or "image/png"
    path = os.path.expanduser(source)
    if not os.path.isfile(path):
        raise ValueError(f"图片文件不存在: {source}")
    ext = os.path.splitext(path)[1].lower()
    mime = IMAGE_TYPES.get(ext, "image/png")
    with open(path, "rb") as f:
        return f.read(), mime


@mcp.tool()
def analyze_image(image: str, question: str = "请详细描述这张图片的内容。") -> str:
    """分析一张图片并回答指定问题。

    适用于：截图诊断、报错识别、界面理解、图表分析等。
    image: 本地图片文件路径（如 /Users/xxx/.reasonix/attachments/a.png）或 data URL。
    question: 针对图片的具体问题（默认要求详细描述）。
    """
    data, mime = _load_image(image)
    return ask_glm_vision(data, mime, question)


@mcp.tool()
def ocr_image(image: str) -> str:
    """提取图片中的全部文字（OCR）。

    适用于：报错截图、代码截图、文档/日志截图。文字原样输出，不翻译不解释。
    image: 本地图片文件路径或 data URL。
    """
    data, mime = _load_image(image)
    return ask_glm_vision(
        data, mime, "请提取图片中的全部文字内容，原样输出，不要翻译、不要解释、不要添加内容。"
    )


@mcp.tool()
def describe_image(image: str) -> str:
    """简要描述图片内容：一句话概括 + 关键细节列表。

    适用于：快速了解界面状态、截图内容、图表结构。
    image: 本地图片文件路径或 data URL。
    """
    data, mime = _load_image(image)
    return ask_glm_vision(
        data, mime, "请用中文简要描述这张图片：先一句话概括，再分点列出关键细节。"
    )


if __name__ == "__main__":
    mcp.run()
