# NovaEnv Vision MCP Server

让**无视觉能力的模型**（如 DeepSeek）通过 MCP 工具「看图」：Reasonix / Cursor 等编辑器把图片传给本服务，本服务调用**智谱 GLM-4.6V-Flash** 多模态 API 返回文字分析，模型据此继续回答用户。

## 原理

```
Reasonix（MCP client，模型如 deepseek 无视觉）
  │  用户粘贴图片 → 模型调用 vision tool（传图片路径）
  ▼
Vision MCP Server（本目录，stdio 启动）
  │  tool: analyze_image / ocr_image / describe_image
  ▼
智谱 GLM-4.6V-Flash（OpenAI 兼容接口）→ 文字描述/OCR 返回
```

## 安装

需要 Python 3.10+：

```bash
cd mcp-vision
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
```

## 配置 API Key

方式一（推荐，环境变量）：
```bash
export ZHIPU_API_KEY=sk-xxxxxxxx
```

方式二（配置文件）：
```json
// ~/.config/novaenv-vision/config.json
{ "api_key": "sk-xxxxxxxx" }
```

Key 申请：https://open.bigmodel.cn/ （智谱开放平台 → API Keys）

## 接入 Reasonix

项目根目录 `.mcp.json`：

```json
{
  "mcpServers": {
    "vision": {
      "command": "python3",
      "args": ["/Users/macos/programs/1024/NovaHub/NovaEnv/mcp-vision/server.py"],
      "env": { "ZHIPU_API_KEY": "sk-xxxxxxxx" }
    }
  }
}
```

重启 Reasonix 后生效。此后你在 Reasonix 中粘贴图片，模型会自动调用 vision 工具识别。

## Tools

| Tool | 说明 |
| --- | --- |
| `analyze_image(image, question)` | 通用看图问答：截图诊断、报错识别、界面理解 |
| `ocr_image(image)` | 提取图片全部文字（报错/代码/日志截图） |
| `describe_image(image)` | 一句话概括 + 关键细节 |

`image` 参数：本地文件绝对路径，或 `data:image/png;base64,...` data URL。

## 命令行自测

```bash
python3 vision.py /path/to/screenshot.png "这个报错是什么"
```

## 说明

- 大图自动压缩（最长边 2048px / 1.5MB）再上传，省流量省费用
- 模型可用 `VISION_MODEL` 环境变量切换（默认 `glm-4.6v-flash`）
- 图片仅上传至智谱 API，本地不落盘、不留存
