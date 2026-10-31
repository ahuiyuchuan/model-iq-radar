"""Build approved two-dimensional pet PNG assets without deforming the character.

Author: ahui. Inputs are the user's four-state and eight-pose design sheets,
plus the approved transparent left-edge head (edge-left-source.png).
Requires Python 3 and Pillow; run `python scripts/build_pet_assets.py` offline.
Use `--edge-only` to rebuild only the head PNG and its optional manifest entry.
Character coordinates are recorded on a 2048-wide working copy. The sheet is
never used as a runtime background. Each pose is isolated, alpha-matted, and
scaled by a measured head span, rather than by its pose's outer bounding box.
Outputs remain schema 1 PNG clips. Extra poses deliberately contain one frame.
Idle uses only a one-source-pixel whole-image translation; no mesh, horizontal
band distortion, fabricated eye cover, scaling, or facial warping is applied.
"""

import argparse
import hashlib
import json
import math
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "assets/pet/blue-whale-maid"
OUTPUT = ROOT / "public/skins/blue-whale-maid"
SIZE = 320
HEAD_WIDTH = 140
FOOT_Y = 304

# Boxes and skull spans use the 2048x1152 working coordinate system. The span
# measures the central head (excluding side flaps, hands, and trailing hair).
# Sitting keeps the same head scale; its feet are not used to enlarge the pose.
POSES = {
    "idle": ("source-actions.png", (75, 85, 450, 520), 207),
    "wave": ("source-actions.png", (580, 85, 950, 520), 205),
    "stretch": ("source-actions.png", (1070, 85, 1465, 520), 203),
    "petting": ("source-actions.png", (1570, 85, 1950, 520), 209),
    "dragging": ("source-actions.png", (75, 545, 445, 1000), 203),
    "landing": ("source-actions.png", (580, 620, 950, 1060), 196),
    "seated": ("source-actions.png", (1080, 630, 1460, 1065), 203),
    "bow": ("source-actions.png", (1570, 615, 1950, 1060), 199),
    "working": ("source-states.png", (80, 20, 650, 600), 261),
    "success": ("source-states.png", (1050, 25, 1625, 600), 273),
    "failed": ("source-states.png", (90, 600, 650, 1130), 253),
    "error": ("source-states.png", (1050, 605, 1625, 1140), 272),
}


def is_backdrop(red, green, blue):
    """Recognize the sheet's cool neutral paper, not warm white garment areas.

    The approved sheet has a low-chroma blue-grey background. A lower luminance
    bound preserves all dark outlines; the upper bound preserves white lace,
    apron and eye highlights. This mask must not be reused for unrelated art.
    """
    return 168 <= red <= 242 and 1 <= green-red <= 15 and 2 <= blue-red <= 27


def extract(sheet, box):
    """Remove neutral paper and disconnected labels; retain the whole character.

    White clothing stays enclosed by its outline, and the color predicate also
    removes enclosed paper between hair strands. Only the largest connected
    silhouette survives: detached result clouds and sheet labels are excluded.
    """
    crop = sheet.crop(box).convert("RGBA")
    width, height = crop.size
    pixels = crop.load()
    candidates = bytearray(width * height)
    for y in range(height):
        for x in range(width):
            r, g, b, _ = pixels[x, y]
            candidates[y*width+x] = is_backdrop(r, g, b)
    # Exterior flood removes paper. Enclosed tiny low-chroma highlights are
    # retained: they belong to lace, apron and eye details, not paper holes.
    # Only a sizeable enclosed paper patch (for example a hair loop) is cleared.
    keep = bytearray([1]) * (width * height)
    inspected = bytearray(width * height)
    for start, candidate in enumerate(candidates):
        if not candidate or inspected[start]:
            continue
        todo = [start]
        inspected[start] = 1
        component = []
        exterior = False
        while todo:
            index = todo.pop()
            component.append(index)
            x, y = index % width, index // width
            exterior = exterior or x in (0,width-1) or y in (0,height-1)
            for nx, ny in ((x-1,y),(x+1,y),(x,y-1),(x,y+1)):
                if 0 <= nx < width and 0 <= ny < height:
                    nxt = ny*width+nx
                    if candidates[nxt] and not inspected[nxt]:
                        inspected[nxt] = 1
                        todo.append(nxt)
        if exterior or len(component) >= 64:
            for index in component:
                keep[index] = 0
    seen = bytearray(width * height)
    largest = []
    for start in range(len(keep)):
        if not keep[start] or seen[start]:
            continue
        todo = [start]
        seen[start] = 1
        component = []
        while todo:
            index = todo.pop()
            component.append(index)
            x, y = index % width, index // width
            for nx, ny in ((x-1,y), (x+1,y), (x,y-1), (x,y+1)):
                if 0 <= nx < width and 0 <= ny < height:
                    nxt = ny*width+nx
                    if keep[nxt] and not seen[nxt]:
                        seen[nxt] = 1
                        todo.append(nxt)
        if len(component) > len(largest):
            largest = component
    if len(largest) < width*height*0.08:
        raise ValueError("No complete character found; refusing to overwrite assets")
    alpha = bytearray(width*height)
    for index in largest:
        alpha[index] = 255
    mask = Image.frombytes("L", (width,height), bytes(alpha))
    # Remove the one-source-pixel matte rim before downsampling. This touches
    # only the silhouette boundary, not interior apron/eye/highlight details.
    mask = mask.filter(ImageFilter.MinFilter(3))
    # Subpixel anti-aliasing is generated by the final high-quality downsample,
    # not by blurring the entire drawing or shrinking white garment details.
    crop.putalpha(mask)
    bounds = mask.getbbox()
    return crop.crop(bounds), (box[0]+bounds[0], box[1]+bounds[1])


def hit_bands(image):
    """Conservatively merge visible alpha into four-source-pixel hit regions."""
    mask = image.getchannel("A").point(lambda value: 255 if value >= 12 else 0)
    mask = mask.filter(ImageFilter.MaxFilter(3))
    bands = []
    for y in range(0, SIZE, 4):
        bounds = mask.crop((0,y,SIZE,min(SIZE,y+4))).getbbox()
        if bounds:
            bands.append([bounds[0],y,bounds[2]-bounds[0],min(4,SIZE-y)])
    return bands


def build_edge(qa_directory=None):
    """Normalize the approved left-edge head without changing painted details.

    The supplied transparent source contains alpha 1--3 noise far outside the
    actual head. Only those nearly invisible pixels are cleared. No RGB color
    key, erosion or white removal is applied, so lace, eyes and hair survive.
    The complete alpha>=4 silhouette is cropped and uniformly resized to fit a
    320px square, touching its left side with four-pixel vertical padding.
    Other directions must not be synthesized by rotating or mirroring this art.
    """
    filename = "edge-left-source.png"
    source = SOURCE / filename
    image = Image.open(source).convert("RGBA")
    original_size = image.size
    alpha = image.getchannel("A").point(lambda value: value if value >= 4 else 0)
    bounds = alpha.getbbox()
    if not bounds or (bounds[2]-bounds[0]) < 100 or (bounds[3]-bounds[1]) < 100:
        raise ValueError("Approved left-edge head is missing or unexpectedly empty")
    image.putalpha(alpha)
    image = image.crop(bounds)
    scale = min((SIZE-4)/image.width, (SIZE-8)/image.height)
    scaled = image.resize((round(image.width*scale), round(image.height*scale)), Image.Resampling.LANCZOS)
    canvas = Image.new("RGBA", (SIZE, SIZE))
    placed_at = (0, (SIZE-scaled.height)//2)
    canvas.alpha_composite(scaled, placed_at)
    # Zero hidden RGB only; preserve antialiasing generated by RGBA downsampling.
    pixels = canvas.load()
    for y in range(SIZE):
        for x in range(SIZE):
            if pixels[x, y][3] == 0:
                pixels[x, y] = (0, 0, 0, 0)
    OUTPUT.mkdir(parents=True, exist_ok=True)
    canvas.save(OUTPUT / "edge-left.png", optimize=True)
    clip = {"atlas":"edge-left.png", "poster":"edge-left.png",
            "frameSize":SIZE, "frames":1, "columns":1,
            "fps":1, "loop":True, "hitBands":hit_bands(canvas)}
    audit = {"author":"ahui", "source":filename,
             "sourceSha256":hashlib.sha256(source.read_bytes()).hexdigest(),
             "sourceSize":list(original_size), "alphaNoiseMaximum":3,
             "sourceCrop":list(bounds), "canvasSize":SIZE, "scale":scale,
             "placedAt":list(placed_at), "visibleBounds":list(canvas.getbbox()),
             "directions":["left"]}
    (SOURCE / "edge-extraction.json").write_text(
        json.dumps(audit, ensure_ascii=False, indent=4)+"\n", encoding="utf-8", newline="\n")
    if qa_directory:
        qa_directory.mkdir(parents=True, exist_ok=True)
        backgrounds = [(255,255,255), (15,28,48), (111,54,94), (32,112,92)]
        board = Image.new("RGB", (SIZE*len(backgrounds), SIZE))
        for index, color in enumerate(backgrounds):
            tile = Image.new("RGBA", (SIZE,SIZE), color+(255,))
            tile.alpha_composite(canvas)
            board.paste(tile.convert("RGB"), (SIZE*index, 0))
        board.save(qa_directory / "edge-left-backgrounds.png")
    return clip


def build(qa_directory=None):
    """Write deterministic PNG clips and an optional multibackground QA board.

    Extra poses have one frame on purpose. Idle has a four-second cycle made
    solely from a one-pixel vertical translation of the complete image: facial
    geometry and body proportions cannot change. There is no invented blink.
    Large posture transitions still need real drawn frames; these assets do not
    claim to supply them.
    """
    OUTPUT.mkdir(parents=True, exist_ok=True)
    sheets = {}
    manifest = {
        "schemaVersion":1, "id":"blue-whale-maid", "name":"蓝鲸女仆",
        "author":"ahui", "description":"依据已确认四状态与八姿态制作的二维桌宠",
        "tokens":{"panelBackground":"#142d4d","panelBorder":"#65b9fb",
                  "primary":"#2d9fff","textPrimary":"#e7f5ff"},
        "decorations":"ocean", "actions":{},
    }
    sprites = {}
    audit = {"author":"ahui", "workingSize":[2048,1152], "canvasSize":SIZE,
             "targetHeadWidth":HEAD_WIDTH, "footBaseline":FOOT_Y, "poses":{}}
    for name,(filename,box,head_width) in POSES.items():
        if filename not in sheets:
            original = Image.open(SOURCE/filename).convert("RGB")
            if original.width/original.height != 16/9:
                raise ValueError("Approved source must keep its 16:9 aspect ratio")
            sheets[filename] = original.resize((2048,1152), Image.Resampling.LANCZOS)
        cutout, origin = extract(sheets[filename],box)
        scale = HEAD_WIDTH/head_width
        scaled = cutout.resize((round(cutout.width*scale),round(cutout.height*scale)), Image.Resampling.LANCZOS)
        if scaled.width > SIZE-8 or scaled.height > SIZE-8:
            raise ValueError(f"{name}: measured pose clips the normalized canvas")
        x = (SIZE-scaled.width)//2
        y = FOOT_Y-scaled.height
        # Seated contact is lower than the head and is intentionally not scaled
        # to the standing height. A future package may supply explicit anchors.
        canvas = Image.new("RGBA",(SIZE,SIZE))
        canvas.alpha_composite(scaled,(x,y))
        # Fully transparent RGB is zeroed so later resizing/compositing cannot
        # accidentally recover the pale paper's hidden color.
        rgba = canvas.load()
        for py in range(SIZE):
            for px in range(SIZE):
                if rgba[px,py][3] == 0:
                    rgba[px,py] = (0,0,0,0)
        canvas.save(OUTPUT/f"{name}.png", optimize=True)
        frames = []
        count = 16 if name == "idle" else 1
        columns = 4 if name == "idle" else 1
        atlas = Image.new("RGBA",(SIZE*columns,SIZE*math.ceil(count/columns)))
        for index in range(count):
            offset = round(math.sin(index*math.tau/count)) if name == "idle" else 0
            frame = Image.new("RGBA",(SIZE,SIZE))
            frame.alpha_composite(canvas,(0,offset))
            atlas.alpha_composite(frame,((index%columns)*SIZE,(index//columns)*SIZE))
            frames.append(frame)
        atlas.save(OUTPUT/f"{name}-atlas.png", optimize=True)
        sprites[name] = canvas
        manifest["actions"][name] = {
            "atlas":f"{name}-atlas.png", "poster":f"{name}.png",
            "frameSize":SIZE, "frames":count,"columns":columns,
            "fps":4 if name == "idle" else 1,
            "loop":True,"hitBands":hit_bands(canvas),
        }
        # Idle only shifts up/down one source pixel. Its conservative regions
        # already include a one-pixel dilation in hit_bands.
        audit["poses"][name] = {"source":filename,"sourceBox":list(box),
            "headSpan":head_width,"scale":scale,"origin":list(origin),
            "placedAt":[x,y],"visibleBounds":list(canvas.getbbox())}
    manifest["docking"] = {"left":build_edge(qa_directory)}
    for name,data in (("skin.json",manifest),):
        (OUTPUT/name).write_text(json.dumps(data,ensure_ascii=False,indent=4)+"\n",encoding="utf-8",newline="\n")
    audit["sources"] = {name:hashlib.sha256((SOURCE/name).read_bytes()).hexdigest() for name in sheets}
    (SOURCE/"extraction.json").write_text(json.dumps(audit,ensure_ascii=False,indent=4)+"\n",encoding="utf-8",newline="\n")
    if qa_directory:
        qa_directory.mkdir(parents=True,exist_ok=True)
        backgrounds = [(255,255,255),(15,28,48),(111,54,94),(32,112,92)]
        board = Image.new("RGB",(SIZE*len(POSES),SIZE*len(backgrounds)))
        draw = ImageDraw.Draw(board)
        for column,(name,sprite) in enumerate(sprites.items()):
            for row,color in enumerate(backgrounds):
                tile = Image.new("RGBA",(SIZE,SIZE),color+(255,))
                tile.alpha_composite(sprite)
                board.paste(tile.convert("RGB"),(column*SIZE,row*SIZE))
                draw.text((column*SIZE+5,row*SIZE+5),name,fill=(190,190,190) if row else (40,40,40))
        board.save(qa_directory/"all-poses-backgrounds.png")
        board.resize((1920,640),Image.Resampling.LANCZOS).save(qa_directory/"all-poses-backgrounds-small.png")
    print(f"Generated {len(POSES)} normalized PNG clips; idle translates one pixel, all extra poses are static")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qa-directory", type=Path, help="Optional output directory for multibackground evidence")
    parser.add_argument("--edge-only", action="store_true", help="Rebuild the approved left-edge resource and manifest without changing pose PNGs")
    args = parser.parse_args()
    if args.edge_only:
        manifest_path = OUTPUT / "skin.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        manifest["docking"] = {"left":build_edge(args.qa_directory)}
        manifest_path.write_text(json.dumps(manifest,ensure_ascii=False,indent=4)+"\n",encoding="utf-8",newline="\n")
        print("Generated approved static left-edge head; existing pose assets unchanged")
    else:
        build(args.qa_directory)
