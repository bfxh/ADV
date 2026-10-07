# 合成语料（片C 试验面）：跨文件流的**消费者**——调用者的参数流进另一文件的汇点。
from f_cross_lib import sink_it


def g(cmd):
    sink_it(cmd)
