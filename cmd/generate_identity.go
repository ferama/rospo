package cmd

import (
	"path/filepath"

	"github.com/ferama/rospo/pkg/utils"
	"github.com/spf13/cobra"
)

func init() {
	rootCmd.AddCommand(generateIdentityCmd)

	generateIdentityCmd.Flags().StringP("path", "p", ".", "key pair destination path")
}

var generateIdentityCmd = &cobra.Command{
	Use:   "identity-gen",
	Short: "Generates private/public key pairs",
	Long:  `Generates private/public key pairs`,
	Run: func(cmd *cobra.Command, args []string) {
		path, _ := cmd.Flags().GetString("path")

		key, err := utils.GeneratePrivateKey()
		if err != nil {
			panic(err)
		}
		publicKey, err := utils.GeneratePublicKey(&key.PublicKey)
		if err != nil {
			panic(err)
		}
		utils.WriteKeyToFile(utils.EncodePrivateKeyToPEM(key), filepath.Join(path, "identity"))
		utils.WriteKeyToFile(publicKey, filepath.Join(path, "identity.pub"))
	},
}
