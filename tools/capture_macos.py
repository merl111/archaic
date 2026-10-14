"""Launch and capture an unsigned Archaic app on the native macOS CI desktop."""
from pathlib import Path
import json, subprocess, sys, time
binary = Path(sys.argv[1]).resolve()
out = Path(sys.argv[2]).resolve()
out.mkdir(parents=True, exist_ok=True)
helper = Path(sys.argv[3]).resolve()
results = []
for name, flags in [('login', ['--theme', 'light']), ('chat-light', ['--fixture', '--theme', 'light']), ('chat-dark', ['--fixture', '--theme', 'dark'])]:
    with (out / (name + '.log')).open('w') as log:
        started = time.monotonic()
        process = subprocess.Popen([str(binary), '--temporary-session', *flags], stdout=log, stderr=log)
        try:
            deadline = time.monotonic() + 60
            while True:
                if process.poll() is not None:
                    raise RuntimeError(f'{name} exited with {process.returncode}; see log')
                found = subprocess.run([str(helper), str(process.pid)], capture_output=True, text=True)
                if found.returncode == 0:
                    window = json.loads(found.stdout)
                    break
                if time.monotonic() >= deadline:
                    subprocess.run(['sample', str(process.pid), '2', '1', '-file', str(out / (name + '-sample.txt'))], timeout=15)
                    subprocess.run([str(helper), str(process.pid), 'all'], stdout=log, stderr=log)
                    subprocess.run(['screencapture', '-x', str(out / (name + '-desktop.png'))], timeout=20)
                    raise RuntimeError(f'{name} has no visible window after 60 seconds; see diagnostics')
                time.sleep(1)
            time.sleep(2)
            wid = str(window['kCGWindowNumber'])
            subprocess.run(['screencapture', '-x', '-o', '-l', wid, str(out / (name + '.png'))], check=True, timeout=20)
            results.append({'scenario': name, 'window': window, 'startup_seconds': round(time.monotonic() - started - 2, 2)})
        finally:
            process.terminate()
            process.wait(timeout=20)
(out / 'windows.json').write_text(json.dumps(results, indent=2))
for flags in [[], ['--fixture']]:
    subprocess.run([str(binary), '--temporary-session', '--smoke-test', *flags], check=True, timeout=40)
print('macOS: login, light and dark chat windows captured; clean startup/shutdown passed')
