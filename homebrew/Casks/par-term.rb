cask "par-term" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.22.0"
  sha256 arm:   "b3724890c4ce2dc56e6e70f154e30b7d1bed45d3da2ba7f7676ee3cb0ea0f36b",
         intel: "55728afb1be3dd9a1a233ec8f757fea58f0190e8ef22baa21d3a33230c6cc483"

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
