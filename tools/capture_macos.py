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
        process = subprocess.Popen([str(binary), '--temporary-session', *flags], stdout=log, stderr=log)
        try:
            time.sleep(6)
            if process.poll() is not None:
                raise RuntimeError(f'{name} exited with {process.returncode}; see log')
            window = json.loads(subprocess.check_output([str(helper), str(process.pid)], text=True))
            wid = str(window['kCGWindowNumber'])
            subprocess.run(['screencapture', '-x', '-o', '-l', wid, str(out / (name + '.png'))], check=True, timeout=20)
            results.append({'scenario': name, 'window': window})
        finally:
            process.terminate()
            process.wait(timeout=20)
(out / 'windows.json').write_text(json.dumps(results, indent=2))
for flags in [[], ['--fixture']]:
    subprocess.run([str(binary), '--temporary-session', '--smoke-test', *flags], check=True, timeout=40)
print('macOS: login, light and dark chat windows captured; clean startup/shutdown passed')
