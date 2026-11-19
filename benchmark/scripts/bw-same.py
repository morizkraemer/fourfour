import json, sys
a = {json.loads(l)['id']: json.loads(l) for l in open(sys.argv[1])}
b = {json.loads(l)['id']: json.loads(l) for l in open(sys.argv[2])}
same = [i for i in a if i in b and a[i]['final_beats_s'] == b[i]['final_beats_s'] and a[i]['bar_phase'] == b[i]['bar_phase'] and a[i].get('beat_mode') == b[i].get('beat_mode')]
diff = [i for i in a if i in b and i not in same]
print('identical', len(same), 'different', len(diff), diff[:20])
