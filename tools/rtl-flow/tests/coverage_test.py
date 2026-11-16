import gzip
import importlib.util
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest

spec = importlib.util.spec_from_file_location('coverage_merge', Path(__file__).parents[1] / 'coverage/merge.py')
merge = importlib.util.module_from_spec(spec)
spec.loader.exec_module(merge)


class CoverageProvenance(unittest.TestCase):
    def record(self, root, name, attributes, compressed=False):
        path = root / 'snps/coverage/db/testdata/run' / name
        path.parent.mkdir(parents=True, exist_ok=True)
        data = f'<test><srctest {attributes}/></test>'.encode()
        path.write_bytes(gzip.compress(data) if compressed else data)

    def test_native_merge_labels_do_not_change_origin(self):
        with tempfile.TemporaryDirectory() as directory:
            a, b = Path(directory) / 'a', Path(directory) / 'b'
            origin = 'starttime="1000" seed="19" cmdline="simulate case_a"'
            self.record(a, 'siminfo.xml', origin, True)
            self.record(b, 'test.xml', origin + ' id="7" name="merged"')
            self.assertEqual(merge.sources(a), merge.sources(b))
            self.record(b, 'test.xml', origin.replace('case_a', 'case_b') + ' id="7" name="merged"')
            self.assertNotEqual(merge.sources(a), merge.sources(b))

    def test_missing_metadata_cannot_be_empty_success(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, 'No native simulation provenance'):
                merge.sources(Path(directory))

    def test_content_change_invalidates_input_fingerprint(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.record(root, 'siminfo.xml', 'seed="1"')
            before = merge.fingerprint(root)
            self.record(root, 'siminfo.xml', 'seed="7"')
            self.assertNotEqual(before, merge.fingerprint(root))

    def test_zero_exit_with_native_error_is_not_success(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            tool = root / 'misleading-tool'
            tool.write_text('#!/bin/sh\nprintf "Error-[BROKEN] merge failed\\n"\nexit 0\n')
            tool.chmod(0o700)
            with self.assertRaisesRegex(RuntimeError, 'URG stage failed'):
                merge.invoke(str(tool), root, [], set(), root / 'output', None)
            self.assertFalse(list((root / 'output').glob('input-*.vdb')))

    def test_overlapping_source_runs_cannot_be_double_counted(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            design, a, b = [root / name for name in ('design', 'a', 'b')]
            design.mkdir()
            self.record(a, 'siminfo.xml', 'cmdline="case_a" seed="1"')
            self.record(b, 'test.xml', 'cmdline="case_a" seed="1" name="merged"')
            inputs = root / 'inputs.list'
            inputs.write_text(f'{a}\n{b}\n')
            args = SimpleNamespace(design=design, inputs=inputs, output=root / 'out', tool='true', batch_size=32)
            with self.assertRaisesRegex(ValueError, 'overlap in original simulation records'):
                merge.merge(args)
            self.assertFalse((root / 'out/merge-result.json').exists())

    def test_zero_exit_with_native_shape_warning_is_not_success(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            tool = root / 'mismatched-tool'
            tool.write_text('#!/bin/sh\nprintf "Warning-[UCAPI-INSTANCE-SHAPEMISMATCH] Instance Data Mismatch\\n"\nexit 0\n')
            tool.chmod(0o700)
            with self.assertRaisesRegex(RuntimeError, 'URG stage failed'):
                merge.invoke(str(tool), root, [], set(), root / 'output', None)

    def test_output_cannot_mutate_an_input_database(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            design, simulation = root / 'design', root / 'simulation'
            design.mkdir()
            self.record(simulation, 'siminfo.xml', 'seed="1"')
            before = merge.fingerprint(simulation)
            inputs = root / 'inputs.list'
            inputs.write_text(f'{simulation}\n')
            args = SimpleNamespace(design=design, inputs=inputs, output=simulation / 'out', tool='true', batch_size=32)
            with self.assertRaisesRegex(ValueError, 'must not overlap an input'):
                merge.merge(args)
            self.assertEqual(before, merge.fingerprint(simulation))


if __name__ == '__main__':
    unittest.main()
