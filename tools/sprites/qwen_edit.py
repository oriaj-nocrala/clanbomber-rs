"""Edición con Qwen-Image 2.1: redibuja una imagen de ComfyUI/input con un prompt.
Uso: qwen_edit.py <workflow_upscale.json> <imagen_en_input> <prefijo> <resolución> <seed>..."""
import json
import sys
import time
import urllib.request

API = "http://127.0.0.1:8188"

PROMPT = (
    "Redraw this video game sprite sheet with much more detail, as high quality modern "
    "hand-painted pixel art. It shows Tux, the Linux penguin mascot, as a game character in a "
    "grid of 10 columns and 4 rows. Keep EXACTLY the same grid, the same number of sprites, "
    "and the same position, size, silhouette and pose of every sprite. "
    "Row 1: Tux facing the viewer (white belly, orange beak and feet). Row 2: Tux facing left "
    "(side view, beak and belly visible on the left). Row 3: Tux seen from behind (black back, "
    "flippers, orange feet). Row 4: Tux facing right (side view). Columns 1 to 9 are a walking "
    "animation. Column 10 is Tux dead, lying flat with a small, well drawn blood splatter. "
    "Glossy black feathers with soft highlights, clean white belly, bright orange beak and "
    "feet, clear dark outline, consistent lighting from the top left. "
    "Flat pure green #00FF00 background, no shadows, no gradients, no grid lines."
)
NEGATIVE = "blurry, lowres, extra sprites, missing sprites, merged sprites, text, watermark"


def post(path, data):
    req = urllib.request.Request(
        API + path, json.dumps(data).encode(), {"Content-Type": "application/json"}
    )
    return json.load(urllib.request.urlopen(req))


wf_path, image, prefix, res, *seeds = sys.argv[1:]
wf = json.load(open(wf_path))
wf["9"]["inputs"]["image"] = image
wf["4"]["inputs"].update(prompt=PROMPT, negative_prompt=NEGATIVE, resolution=int(res))
wf["8"]["inputs"]["filename_prefix"] = prefix

ids = []
for s in seeds:
    wf["6"]["inputs"]["seed"] = int(s)
    ids.append(post("/prompt", {"prompt": wf})["prompt_id"])
    print("encolado", s, flush=True)
for pid in ids:
    while True:
        h = json.load(urllib.request.urlopen(f"{API}/history/{pid}"))
        if pid in h and h[pid].get("status", {}).get("completed") is not None:
            files = [i["filename"] for o in h[pid]["outputs"].values() for i in o.get("images", [])]
            print(pid, h[pid]["status"].get("status_str"), files, flush=True)
            break
        time.sleep(5)
