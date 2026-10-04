"""FP 会计辅助：VF3 扫描结果的测试/产品分账与产品 panic 清单（M1 片1，一次性脚手架，白名单见 spec/PY-WHITELIST.md）。"""
import json, sys, collections

rows = [json.loads(l) for l in open(sys.argv[1], encoding='utf-8')]

def norm(f: str) -> str:
    return f.replace('\\', '/')

def is_test(f: str) -> bool:
    n = norm(f)
    return '/tests/' in n or n.endswith('/tests.rs') or '/benches/' in n or '/examples/' in n

c = collections.Counter((r['rule'], is_test(r['file'])) for r in rows)
for (rule, test), n in sorted(c.items()):
    print(f"{'TEST' if test else 'PROD'} {rule}: {n}")

print('---PROD panics---')
for r in rows:
    if r['rule'] == 'RS-PANIC-USE' and not is_test(r['file']):
        print(f"{norm(r['file'])}:{r['start_line']}")

print('---PROD unwrap top files---')
uf = collections.Counter(norm(r['file']) for r in rows if r['rule'] == 'RS-UNWRAP-USE' and not is_test(r['file']))
for k, v in uf.most_common(10):
    print(v, k)
print(f"PROD unwrap 文件数: {len(uf)}, 总数: {sum(uf.values())}")
