cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.24.0"
  sha256 arm:   "992434cbc5a78837167cf899f6adce41b3bcb9866ec93d5ed0855939ef620b09",
         intel: "12bb736622b7416f72dcc61368d461bfa318f9c0e7776cb94e94bf8b691da189"

  url "https://github.com/paulrobello/par-term/releases/download/v#{version}/par-term-macos-#{arch}.zip"
  name "par-term"
  desc "Cross-platform GPU-accelerated terminal emulator with inline graphics support"
  homepage "https://github.com/paulrobello/par-term"

  depends_on macos: ">= :catalina"

  livecheck do
    url :homepage
    strategy :github_latest
  end

  app "par-term.app"

  zap trash: [
    "~/Library/Application Support/par-term",
    "~/Library/Preferences/com.paulrobello.par-term.plist",
    "~/Library/Saved Application State/com.paulrobello.par-term.savedState",
    "~/.config/par-term",
  ]
end
