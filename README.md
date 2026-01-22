# Request-TUI

TUI界面的并发下载器

**施工中...**

## 安装方式

使用如下命令编译：

```shell
cargo build --release
```

使用如下命令安装：
```shell
cargo install --path .
```

## TODO

- [ ] 出现问题时的弹窗提示
- [ ] 设置目录以及相关设置
- [ ] 使用`?`打开帮助对话框
- [ ] 鼠标支持

## 已知问题

当同时下载同名文件时，由于多线程竞争问题，导致创建同名文件的问题。（见[resolve.rs#L172](./src/app/task/resolve.rs)）

