import os, sys, unittest
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))
from game_export import as3_to_js, export

class T(unittest.TestCase):
    def test_as3(self):
        self.assertEqual(as3_to_js('var n:int = 1; trace(n);'), 'let n = 1; console.log(n);')

    def test_export(self):
        h = export({'width': 10, 'height': 10, 'sprites': [
            {'name': 'a', 'x': 0, 'y': 0, 'w': 1, 'h': 1, 'onEnterFrame': 'var v:Number=1;this.x+=v;'}]})
        self.assertIn('<canvas', h)
        self.assertIn('let v=1', h.replace(' ', ''))
        self.assertNotIn('__SCENE__', h)

if __name__ == '__main__':
    unittest.main()
