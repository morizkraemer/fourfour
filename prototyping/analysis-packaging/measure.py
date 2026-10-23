import resource
import subprocess
import sys
import time

out_path, err_path, *cmd = sys.argv[1:]
start = time.monotonic()
with open(out_path, "wb") as out, open(err_path, "wb") as err:
    code = subprocess.run(cmd, stdout=out, stderr=err).returncode
wall = time.monotonic() - start
ru = resource.getrusage(resource.RUSAGE_CHILDREN)
print(f"exit={code} wall={wall:.2f}s cpu={ru.ru_utime + ru.ru_stime:.2f}s peak_rss={ru.ru_maxrss / 1024:.0f}MB")
sys.exit(code)
