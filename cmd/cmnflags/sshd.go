package cmnflags

import (
	"github.com/ferama/rospo/pkg/sshd"
	"github.com/spf13/cobra"
	"github.com/spf13/pflag"
)

// AddSshdFlags adds sshd common flags to FlagSet
func AddSshdFlags(fs *pflag.FlagSet) {
	fs.StringP("sshd-authorized-keys", "K", "./authorized_keys", "ssh server authorized keys path.\nhttp url like https://github.com/<username>.keys are supported too")
	fs.StringP("sshd-listen-address", "P", ":2222", "the ssh server tcp port")
	fs.StringP("sshd-key", "I", "./server_key", "the ssh server key path")
	fs.BoolP("disable-auth", "T", false, "if set clients can connect without authentication")
	fs.StringP("sshd-authorized-password", "A", "", "ssh server authorized password. Disabled if empty")
}

// GetSshDConf builds an SshDConf object from cmd
func GetSshDConf(cmd *cobra.Command) *sshd.SshDConf {
	sshdKey, _ := cmd.Flags().GetString("sshd-key")
	sshdAuthorizedKeys, _ := cmd.Flags().GetString("sshd-authorized-keys")
	sshdListenAddress, _ := cmd.Flags().GetString("sshd-listen-address")
	authorizedPasssword, _ := cmd.Flags().GetString("sshd-authorized-password")
	disableAuth, _ := cmd.Flags().GetBool("disable-auth")

	return &sshd.SshDConf{
		Key:                sshdKey,
		AuthorizedKeysURI:  []string{sshdAuthorizedKeys},
		ListenAddress:      sshdListenAddress,
		AuthorizedPassword: authorizedPasssword,
		DisableAuth:        disableAuth,
	}
}
