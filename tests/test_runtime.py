"""Device-free integration checks for the actual Rust JSON-lines process."""
import json
import os
from pathlib import Path
import queue
import subprocess
import tempfile
import threading
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get("OMARSKET_TEST_BIN", ROOT / "target/debug" /
                            ("omarsket.exe" if os.name == "nt" else "omarsket")))


class Session:
    def __init__(self, folder):
        self.counter, self.replies = 0, queue.Queue()
        self.process = subprocess.Popen([str(BINARY), "session", "--dry-run", "--library", str(folder / "library.json")],
                                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        stderr=subprocess.PIPE, text=True, encoding="utf-8")
        def reader():
            for line in self.process.stdout:
                self.replies.put(json.loads(line))
        self.reader = threading.Thread(target=reader)
        self.reader.start()
        assert self.replies.get(timeout=5)["event"] == "ready"

    def call(self, method, **kwargs):
        self.counter += 1
        request = dict(method=method, id=self.counter, **kwargs)
        self.process.stdin.write(json.dumps(request) + "\n")
        self.process.stdin.flush()
        while True:
            reply = self.replies.get(timeout=5)
            if reply.get("id") == self.counter:
                return reply

    def ok(self, method, **kwargs):
        reply = self.call(method, **kwargs)
        assert reply["ok"], reply
        return reply["result"]

    def close(self):
        if self.process.poll() is None:
            self.ok("shutdown")
            self.process.stdin.close()
            self.process.wait(timeout=5)
        self.reader.join(timeout=5)
        self.process.stdout.close()
        self.process.stderr.close()


@unittest.skipUnless(BINARY.is_file(), "Run cargo build before integration tests")
class RuntimeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="omarsket-tests-")
        self.folder = Path(self.temp.name)
        self.session = Session(self.folder)

    def tearDown(self):
        self.session.close()
        self.temp.cleanup()

    def save(self, name="test", actions=None, **kwargs):
        task = dict(id=name, actions=actions if actions is not None else [dict(type="wait", duration=0)], **kwargs)
        self.session.ok("save", task=task)

    def test_all_native_types_roundtrip_and_import_permissions(self):
        actions = [dict(type="paste", content="Unicode λ"), dict(type="wait", duration=0),
                   dict(type="keyssequence", keysmap={"0": [10, ["CONTROL_LEFT", "C"]]}),
                   dict(type="cursormovements", cursormovsmap=[[0, 10, -20, 100]]),
                   dict(type="systemcommand", sysCommandType="shutdown", sysCommandParam1="Don't force"),
                   dict(type="runningothertask", otherTaskName="child", otherTaskLoops=1)]
        path = self.folder / "task.scht"
        path.write_text(json.dumps(dict(docType="ScheduleTask File", actions=actions)), encoding="utf-8")
        self.session.ok("import", name="imported", path=str(path))
        exported = self.session.ok("export", name="imported", format="tasket")
        self.assertEqual(len(exported["actions"]), 6)
        self.assertEqual(exported["actions"][4]["sysCommandParam1"], "Don't force")
        task = self.session.ok("snapshot")["tasks"][0]
        self.assertFalse(task["allow_system"])
        self.assertFalse(task["auto_start"])

    def test_atomic_validation_and_persistence(self):
        self.save()
        self.assertFalse(self.session.call("save", task=dict(id="test", actions=[dict(type="invalid")]))["ok"])
        self.session.close()
        self.session = Session(self.folder)
        self.assertEqual(self.session.ok("snapshot")["tasks"][0]["actions"][0]["type"], "wait")

    def test_nested_loops_and_completion(self):
        self.save("child")
        self.save("parent", [dict(type="runningothertask", otherTaskName="child", otherTaskLoops=2)], loop_times=3)
        self.session.ok("run", name="parent")
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            job = self.session.ok("snapshot")["jobs"][0]
            if job["state"] == "finished": break
            time.sleep(0.01)
        self.assertEqual(job["state"], "finished")
        self.assertEqual(job["completed_loops"], 3)

    def test_stop_running_and_queued_tasks(self):
        self.save("slow", [dict(type="wait", duration=60)])
        self.save("next")
        self.session.ok("run", name="slow")
        self.session.ok("run", name="next")
        start = time.monotonic()
        self.session.ok("stop")
        self.assertLess(time.monotonic() - start, 1)
        self.assertTrue(all(j["state"] == "stopped" for j in self.session.ok("snapshot")["jobs"]))

    def test_rejects_cycles_and_unapproved_commands(self):
        self.save("cycle", [dict(type="runningothertask", otherTaskName="cycle")])
        self.assertFalse(self.session.call("run", name="cycle")["ok"])
        self.save("command", [dict(type="systemcommand", sysCommandType="executeprogram", sysCommandParam1="unused")])
        self.assertIn("permission", self.session.call("run", name="command")["error"])
        self.assertEqual(self.session.ok("snapshot")["jobs"], [])

    def test_data_resolution_rescale_and_non_overwrite(self):
        self.session.ok("save-data", name="path", item=dict(kind="cursormovements", value=[[3, 20, -100, 50]]))
        self.save("path-task", [dict(type="cursormovements", cursorMovsId="path", cursormovsmap=[])])
        self.session.ok("rescale", name="path-task", old=[100, 100], new=[200, 300])
        snapshot = self.session.ok("snapshot")
        action = snapshot["tasks"][0]["actions"][0]
        self.assertEqual(action["cursormovsmap"], [[3, 20, -200, 150]])
        self.assertEqual(action["cursorMovsId"], "")
        self.assertEqual(snapshot["data"]["path"]["value"], [[3, 20, -100, 50]])
        output = self.folder / "export.json"
        self.session.ok("export", name="path-task", path=str(output))
        self.assertFalse(self.session.call("export", name="path-task", path=str(output))["ok"])

    def test_delayed_task_can_be_cancelled_individually(self):
        self.save()
        a = self.session.ok("run", name="test", delay=60)
        b = self.session.ok("run", name="test", delay=60)
        self.session.ok("stop", number=a["number"])
        jobs = self.session.ok("snapshot")["jobs"]
        self.assertEqual(jobs[0]["state"], "stopped")
        self.assertEqual(jobs[1]["number"], b["number"])
        self.assertEqual(jobs[1]["state"], "scheduled")

    def test_launcher_escapes_exec_field_codes(self):
        self.save("100% ready")
        output = self.folder / "launch.desktop"
        self.session.ok("shortcut", name="100% ready", path=str(output))
        self.assertIn('run "100%% ready"', output.read_text(encoding="utf-8"))

    def test_windows_export_does_not_silently_drop_extensions(self):
        self.save("extended", [dict(type="scroll", vertical=1)])
        result = self.session.call("export", name="extended", format="tasket")
        self.assertFalse(result["ok"])
        self.assertIn("Omarsket-only", result["error"])


if __name__ == "__main__": unittest.main()
