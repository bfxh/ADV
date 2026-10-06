# 合成语料（试验面，登记在 spec/maturity.json）：故意写 eval/exec 给快轨规则当命中目标。
# 这里没有真实执行语义，删掉这两行等于删掉 report_counts.rs 的判据。
def a(cmd):
    eval(cmd)
    return eval("1 + 1")
