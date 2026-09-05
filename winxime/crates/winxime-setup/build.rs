fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("../../resources/icon.ico");
        // 显式声明 asInvoker：文件名含 "setup" 会触发 Windows UAC 安装程序检测，
        // 被默认要求提权（CreateProcess 报 os error 740），导致普通权限的 server 无法拉起设置窗口。
        res.set_manifest_file("winxime-setup.manifest");
        res.compile().expect("Failed to embed icon");
    }
}
