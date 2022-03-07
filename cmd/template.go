package cmd

import (
	"embed"
	"fmt"

	"github.com/spf13/cobra"
)

//go:embed configs/config_template.yaml
var configTemlplate embed.FS

func init() {
	rootCmd.AddCommand(templateCmd)
}

var templateCmd = &cobra.Command{
	Use:   "template",
	Short: "Generates a config template file",
	Long:  `Generates a config template file`,
	Example: `
  # generates a template and store it into the conf.yaml file
  $ rospo template > conf.yaml
	`,
	Run: func(cmd *cobra.Command, args []string) {
		content, _ := configTemlplate.ReadFile("configs/config_template.yaml")
		fmt.Println(string(content))
	},
}
