import importlib.util
import pathlib
import unittest


SKILL_DIR = pathlib.Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("wechat_binance_skill", SKILL_DIR / "SKILL.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class WechatBinanceSkillTests(unittest.TestCase):
    def test_parse_invocation_preserves_stdin_payload(self):
        contact, top = MODULE.parse_invocation([], '{"contact":"发财群","top":5}', "{}")
        self.assertEqual((contact, top), ("发财群", 5))

    def test_validate_top_rejects_values_outside_documented_range(self):
        for value in (-1, 0, 101):
            with self.subTest(value=value):
                with self.assertRaises(ValueError):
                    MODULE.validate_top(value)
        self.assertEqual(MODULE.validate_top(100), 100)

    def test_wechat_script_verifies_selected_conversation_before_sending(self):
        script = MODULE.build_wechat_script("发财群", "hello")
        self.assertIn('if selectedConversation is not "发财群" then', script)
        self.assertIn('error "联系人校验失败', script)
        self.assertNotIn("random number", script)

    def test_metadata_declares_actual_privileges(self):
        metadata = (SKILL_DIR / "meta.yaml").read_text(encoding="utf-8")
        for permission in ("network", "system.exec", "system.automation", "system.accessibility"):
            self.assertIn(f"- {permission}", metadata)


if __name__ == "__main__":
    unittest.main()
