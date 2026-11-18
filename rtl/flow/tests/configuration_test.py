"""One profile must determine RTL, verification and implementation geometry."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

SOURCE = Path(__file__).resolve().parents[1] / 'configuration.py'
spec = importlib.util.spec_from_file_location('configuration', SOURCE)
configuration = importlib.util.module_from_spec(spec)
spec.loader.exec_module(configuration)


class ConfigurationTest(unittest.TestCase):
    def test_geometry_is_derived(self):
        c = configuration.resolve({'channels': 3, 'memory_bits': 128, 'burst_beats': 4,
                                   'slots': 5, 'read_outstanding': 2, 'write_outstanding': 7})
        self.assertEqual((c['memory_bytes'], c['burst_bytes'], c['row_bits'], c['rows'], c['slot_bits']),
                         (16, 64, 512, 15, 3))
        self.assertEqual((c['read_outstanding'], c['write_outstanding']), (2, 7))

    def test_dependent_parameters_cannot_diverge(self):
        for extra in ({'rows': 33}, {'burst_bytes': 64}, {'slot_bits': 4}, {'typo': 1}):
            with self.subTest(extra=extra), self.assertRaises(ValueError):
                configuration.resolve(extra)

    def test_illegal_profiles_fail_before_tools(self):
        for extra in ({'channels': 0}, {'channels': 9}, {'slots': 0}, {'register_bits': 16},
                      {'memory_bits': 96}, {'memory_bits': 2048}, {'read_outstanding': 0},
                      {'write_outstanding': False}, {'request_depth': 3}, {'receipt_depth': 1},
                      {'burst_beats': 257}, {'memory_bits': 32, 'burst_beats': 1}):
            with self.subTest(extra=extra), self.assertRaises(ValueError):
                configuration.resolve(extra)

    def test_profile_requires_an_object(self):
        for value in ([], [1, 2], False, 0, "channels"):
            with self.subTest(value=value), self.assertRaises(ValueError):
                configuration.resolve(value)

    def test_queue_depth_respects_pinned_cell_constraint_contract(self):
        for key in ('request_depth', 'receipt_depth'):
            with self.subTest(key=key), self.assertRaises(ValueError):
                configuration.resolve({key: 2})
        c = configuration.resolve({'request_depth': 4, 'receipt_depth': 16})
        self.assertEqual((c['request_depth'], c['receipt_depth']), (4, 16))

    def test_manifest_selects_real_sources_and_types(self):
        with tempfile.TemporaryDirectory() as tmp:
            dst = Path(tmp) / 'project'
            c = configuration.resolve({'channels': 8, 'register_bits': 64, 'memory_bits': 128, 'slots': 3})
            configuration.project(dst, c)
            native = (dst / 'src/profile.veryl').read_text()
            self.assertIn('DmaCopy::<', native)
            self.assertIn('CHANNEL_COUNT: 8', native)
            self.assertIn('64, 16, 1, 35, 1, 2, 35, 2', native)
            self.assertIn('32, 8, 1', native)
            self.assertIn('"channels": 8', (dst / 'configuration.json').read_text())
            self.assertFalse((dst / 'src/dma').exists(), 'must not duplicate design sources')
            with self.assertRaises(FileExistsError):
                configuration.project(dst, c)


if __name__ == '__main__':
    unittest.main()
