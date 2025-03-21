package autocomplete

import (
	"github.com/ferama/rospo/pkg/utils"
	"github.com/spf13/cobra"
)

// Test with:
//
//	go build . && eval "$(./rospo completion zsh)"
//	./rospo shell <tab> <tab>
func Host() func(cmd *cobra.Command, args []string, toComplete string) ([]string, cobra.ShellCompDirective) {
	cp := utils.GetSSHConfigInstance()

	return func(cmd *cobra.Command, args []string, toComplete string) ([]string, cobra.ShellCompDirective) {
		shellDirective := cobra.ShellCompDirectiveNoFileComp | cobra.ShellCompDirectiveNoSpace

		return cp.GetHostNames(), shellDirective
	}
}
