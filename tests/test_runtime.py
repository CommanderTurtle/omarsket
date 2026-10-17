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

    def restart(self):
        self.session.close()
        self.session = Session(self.folder)

    def test_individual_files_only_rewrite_changed_schedule(self):
        self.save("a")
        self.save("b")
        a, b = (self.folder / "schedules" / f"{name}.omscht" for name in ("a", "b"))
        before = b.stat().st_mtime_ns
        self.save("a", description="changed")
        self.assertEqual(json.loads(a.read_text(encoding="utf-8"))["description"], "changed")
        self.assertEqual(b.stat().st_mtime_ns, before)
        self.assertFalse((self.folder / "library.json").exists())
        self.session.ok("delete", name="a")
        self.assertFalse(a.exists())
        self.assertTrue(b.exists())

    def test_scht_auto_conversion_preserves_source_and_does_not_resurrect(self):
        source = self.folder / "schedules" / "legacy.scht"
        original = json.dumps(dict(docType="ScheduleTask File", actions=[dict(type="wait", duration=0)],
                                   auto_start=True, allow_system=True))
        source.write_text(original, encoding="utf-8")
        self.restart()
        native = source.with_suffix(".omscht")
        self.assertTrue(native.is_file())
        task = self.session.ok("snapshot")["tasks"][0]
        self.assertFalse(task["auto_start"])
        self.assertFalse(task["allow_system"])
        self.assertEqual(self.session.ok("snapshot")["jobs"], [])
        self.assertEqual(source.read_text(encoding="utf-8"), original)
        self.session.ok("delete", name="legacy")
        self.restart()
        self.assertEqual(self.session.ok("snapshot")["tasks"], [])
        self.assertFalse(native.exists())

    def test_legacy_library_migrates_once_and_keeps_existing_schedules(self):
        self.save("existing", description="newer")
        legacy = dict(tasks={
            "existing": dict(id="existing", description="old", actions=[]),
            "old": dict(id="old", delay_seconds=10, loop_times=2, actions=[dict(type="wait", duration=0)])},
            data={"text": dict(kind="paste", value="saved data")})
        original = json.dumps(legacy)
        (self.folder / "library.json").write_text(original, encoding="utf-8")
        self.restart()
        s = self.session.ok("snapshot")
        self.assertEqual({t["id"] for t in s["tasks"]}, {"existing", "old"})
        self.assertEqual(next(t for t in s["tasks"] if t["id"] == "existing")["description"], "newer")
        self.assertEqual(s["data"]["text"]["value"], "saved data")
        self.assertTrue((self.folder / "data.json").exists())
        self.session.ok("delete", name="old")
        self.restart()
        self.assertEqual([t["id"] for t in self.session.ok("snapshot")["tasks"]], ["existing"])
        self.assertEqual((self.folder / "library.json").read_text(encoding="utf-8"), original)

    def test_rename_updates_native_and_tasket_nested_references(self):
        self.save("child")
        self.save("parent", [dict(type="runningothertask", otherTaskName="child.omscht")])
        child = next(t for t in self.session.ok("snapshot")["tasks"] if t["id"] == "child")
        child["id"] = "renamed"
        self.session.ok("save", task=child, previous="child")
        self.assertFalse((self.folder / "schedules/child.omscht").exists())
        self.assertTrue((self.folder / "schedules/renamed.omscht").exists())
        parent = next(t for t in self.session.ok("snapshot")["tasks"] if t["id"] == "parent")
        self.assertEqual(parent["actions"][0]["otherTaskName"], "renamed")
        self.session.ok("run", name="parent")
        self.assertFalse(self.session.call("duplicate", name="parent", new_name="renamed")["ok"])
        self.session.ok("duplicate", name="parent", new_name="copy")
        self.assertTrue((self.folder / "schedules/copy.omscht").exists())

    def test_temporary_schedule_does_not_save_or_change_saved_task(self):
        self.save("saved")
        before = (self.folder / "schedules/saved.omscht").read_bytes()
        job = self.session.ok("run-temporary", task=dict(id="scratch", delay_seconds=60,
                              loop_times=5, actions=[dict(type="wait", duration=0)]))
        self.assertTrue(job["temporary"])
        self.assertEqual(job["loop_times"], 5)
        self.assertEqual(job["total_actions"], 1)
        self.assertFalse((self.folder / "schedules/scratch.omscht").exists())
        self.assertEqual((self.folder / "schedules/saved.omscht").read_bytes(), before)
        self.session.ok("stop", number=job["number"])

    def test_native_export_embeds_shared_data_and_reimport_resets_permissions(self):
        self.session.ok("save-data", name="text", item=dict(kind="paste", value="resolved"))
        self.save("native", [dict(type="paste", content="fallback", contentId="text")],
                  auto_start=True, allow_system=True, delay_seconds=3, loop_times=4)
        out = self.folder / "portable.omscht"
        self.session.ok("export", name="native", format="omarsket", path=str(out))
        self.assertEqual(json.loads(out.read_text(encoding="utf-8"))["actions"][0]["content"], "resolved")
        self.session.ok("import", name="imported", path=str(out))
        task = next(t for t in self.session.ok("snapshot")["tasks"] if t["id"] == "imported")
        self.assertEqual(task["delay_seconds"], 3)
        self.assertEqual(task["loop_times"], 4)
        self.assertFalse(task["auto_start"])
        self.assertFalse(task["allow_system"])

    def test_collection_roundtrip_remains_individual_and_no_overwrite(self):
        self.save("one", auto_start=True, allow_system=True)
        self.save("two")
        self.session.ok("save-data", name="shared", item=dict(kind="paste", value="portable"))
        out = self.folder / "collection.json"
        self.session.ok("export-library", path=str(out))
        self.assertFalse(self.session.call("import-library", path=str(out))["ok"])
        other = self.folder / "other"
        other.mkdir()
        s = Session(other)
        try:
            s.ok("import-library", path=str(out))
            self.assertEqual(len(list((other / "schedules").glob("*.omscht"))), 2)
            self.assertFalse((other / "library.json").exists())
            self.assertFalse(any(t["auto_start"] or t["allow_system"] for t in s.ok("snapshot")["tasks"]))
        finally:
            s.close()

    def test_folder_import_is_validated_before_any_write(self):
        folder = self.folder / "imports"
        folder.mkdir()
        (folder / "good.scht").write_text(json.dumps(dict(docType="ScheduleTask File", actions=[])), encoding="utf-8")
        bad = folder / "bad.omscht"
        bad.write_text("invalid", encoding="utf-8")
        self.assertFalse(self.session.call("import-folder", path=str(folder))["ok"])
        self.assertEqual(self.session.ok("snapshot")["tasks"], [])
        bad.write_text(json.dumps(dict(id="ignored", actions=[])), encoding="utf-8")
        self.assertEqual(self.session.ok("import-folder", path=str(folder))["imported"], 2)
        self.assertEqual({p.stem for p in (self.folder / "schedules").glob("*.omscht")}, {"good", "bad"})

    def test_external_edit_is_not_silently_overwritten(self):
        self.save("external")
        path = self.folder / "schedules/external.omscht"
        original = json.loads(path.read_text(encoding="utf-8"))
        changed = dict(original, description="outside")
        path.write_text(json.dumps(changed), encoding="utf-8")
        self.assertIn("changed on disk", self.session.call("save", task=dict(original, description="inside"))["error"])
        self.assertEqual(json.loads(path.read_text(encoding="utf-8"))["description"], "outside")
        self.assertEqual(self.session.ok("reload")["tasks"][0]["description"], "outside")

    def test_native_file_with_omitted_defaults_can_be_edited(self):
        path = self.folder / "schedules/minimal.omscht"
        path.write_text(json.dumps(dict(id="minimal", actions=[])), encoding="utf-8")
        self.restart()
        self.save("minimal", description="editable")
        self.assertEqual(json.loads(path.read_text(encoding="utf-8"))["description"], "editable")

    def test_preferences_persist_without_touching_schedules(self):
        self.save()
        path = self.folder / "schedules/test.omscht"
        before = path.stat().st_mtime_ns
        self.session.ok("preference", name="auto_scroll", value=False)
        self.assertFalse(self.session.call("preference", name="unknown", value=True)["ok"])
        self.restart()
        self.assertFalse(self.session.ok("snapshot")["preferences"]["auto_scroll"])
        self.assertEqual(path.stat().st_mtime_ns, before)


if __name__ == "__main__": unittest.main()
