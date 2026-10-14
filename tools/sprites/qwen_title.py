"""Manda el workflow de Qwen-Image 2.1 (GGUF, lowvram) a ComfyUI y espera las imágenes.
Uso: qwen_title.py <workflow.json> <prefijo> <seed> [<seed>...]"""
import json
import sys
import time
import urllib.request

API = "http://127.0.0.1:8188"

PROMPT = (
    "Title screen artwork for a retro arcade bomberman video game. "
    "Black background. In the center, the game logo text \"ClanBomber 2\" in large elegant "
    "chrome italic script letters with dark red reflections and a glowing white-hot rim, "
    "\"ClanBomber\" on one line and a big glowing fiery orange \"2\" at the lower right. "
    "Behind the logo a large dark red five-pointed star, and swirling wisps of fire and "
    "flame trails in red, orange and yellow rising above it like an explosion. "
    "Small sparks and embers. Dramatic lighting, high contrast, clean dark edges around the "
    "image, digital art, sharp details. No other text."
)
NEGATIVE = "watermark, signature, extra text, misspelled text, blurry, lowres, frame, border"


def post(path, data):
    req = urllib.request.Request(
        API + path, json.dumps(data).encode(), {"Content-Type": "application/json"}
    )
    return json.load(urllib.request.urlopen(req))


def get(path):
    return json.load(urllib.request.urlopen(API + path))


wf_path, prefix, *seeds = sys.argv[1:]
wf = json.load(open(wf_path))
wf["4"]["inputs"]["prompt"] = PROMPT
wf["4"]["inputs"]["negative_prompt"] = NEGATIVE
wf["5"]["inputs"].update(width=1152, height=864)
wf["8"]["inputs"]["filename_prefix"] = prefix

ids = []
for s in seeds:
    wf["6"]["inputs"]["seed"] = int(s)
    ids.append(post("/prompt", {"prompt": wf})["prompt_id"])
    print("encolado", s, flush=True)

for pid in ids:
    while True:
        h = get("/history/" + pid)
        if pid in h and h[pid].get("status", {}).get("completed") is not None:
            st = h[pid]["status"]
            files = [i["filename"] for o in h[pid]["outputs"].values() for i in o.get("images", [])]
            print(pid, st.get("status_str"), files, flush=True)
            break
        time.sleep(5)
