import json,sys
tr=json.load(open(sys.argv[1]))["tracks"]
for t in sorted(tr,key=lambda t:t["ref_bpm"]):
    if t["ref_bpm"]>=148 or t["ref_bpm"]<82:
        print(t["id"],round(t["ref_bpm"],1),round(t["est_bpm"],1),[round(x,2) for x in t["hb"]])
