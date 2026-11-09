// Yardımcı neden kullanılamıyor ve nasıl düzeltilir.
import type { HelperStatus } from "../api";
import { useT, type Key } from "../i18n";
import { Command } from "./Icon";

const PATH = "/usr/lib/bosun/bosun-helper";

export function HelperNotice({ status }: { status: HelperStatus }) {
  const { t } = useT();
  if (status === "ok") return null;
  const fixPerms = `sudo chown root:root ${PATH} && sudo chmod 755 ${PATH}`;
  return (
    <div className="callout note">
      <p style={{ margin: "0 0 6px" }}>{t(`helper.${status}` as Key, { path: PATH })}</p>
      {status === "missing" || status === "not_a_file" ? (
        <>
          <Command text="sudo apt install --reinstall ./Bosun_0.4.0_amd64.deb" />
          <p style={{ margin: "8px 0 4px", fontSize: 13, color: "var(--ink-3)" }}>{t("helper.fromSource")}</p>
          <Command text="./install.sh --helper" />
        </>
      ) : (
        <Command text={fixPerms} />
      )}
    </div>
  );
}
