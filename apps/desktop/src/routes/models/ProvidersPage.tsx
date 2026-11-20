import { useState } from 'react';
import { Icon } from '@iconify/react';
import plusIcon from '@iconify-icons/lucide/plus';
import { useTranslation } from '@sonasub/i18n';
import { Badge } from '@sonasub/ui/components/badge';
import { Button } from '@sonasub/ui/components/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@sonasub/ui/components/card';
import { useQuery } from '@tanstack/react-query';

import { ProviderDialog } from '@/components/provider-dialog';
import { errorText, providersOptions } from '@/lib/providers';
import type { ProviderConfig } from '@/lib/providers';

export function ProvidersPage() {
  const { t } = useTranslation();
  const { data, isPending, isError, error } = useQuery(providersOptions);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editing, setEditing] = useState<ProviderConfig | null>(null);

  function openCreate() {
    setEditing(null);
    setDialogOpen(true);
  }

  function openEdit(provider: ProviderConfig) {
    setEditing(provider);
    setDialogOpen(true);
  }

  return (
    <div className="h-full overflow-hidden">
      <div className="mx-auto flex h-full w-full max-w-[1280px] flex-col gap-4 overflow-y-auto p-4 md:p-5">
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-1">
            <h1 className="text-lg font-semibold">{t(($) => $.models.providers.title)}</h1>
            <p className="text-muted-foreground">{t(($) => $.models.providers.description)}</p>
          </div>
          <Button type="button" size="sm" onClick={openCreate}>
            <Icon icon={plusIcon} />
            {t(($) => $.models.providers.add)}
          </Button>
        </div>

        {isPending && <p className="text-muted-foreground">{t(($) => $.models.loading)}</p>}
        {isError && <p className="text-destructive">{errorText(error)}</p>}
        {data?.loadError && <p className="text-destructive">{data.loadError}</p>}
        {data?.providers.length === 0 && (
          <Card>
            <CardHeader>
              <CardTitle>{t(($) => $.models.providers.empty)}</CardTitle>
              <CardDescription>{t(($) => $.models.providers.emptyHint)}</CardDescription>
            </CardHeader>
          </Card>
        )}
        {data?.providers.map((provider) => (
          <Card key={provider.name} className="gap-2 py-4">
            <CardHeader className="px-4">
              <div className="flex items-start justify-between gap-3">
                <div>
                  <CardTitle className="text-[14px]">{provider.name}</CardTitle>
                  <CardDescription className="break-all text-xs">
                    {provider.baseUrl}
                  </CardDescription>
                </div>
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  onClick={() => openEdit(provider)}
                >
                  {t(($) => $.models.providers.edit)}
                </Button>
              </div>
            </CardHeader>
            <CardContent className="flex flex-wrap gap-2 px-4 text-xs">
              <Badge variant="secondary">
                {t(($) => $.models.providers.modelsCount, { count: provider.models.length })}
              </Badge>
              <Badge variant="outline">
                {t(($) => $.models.providers.concurrency, { count: provider.concurrency })}
              </Badge>
            </CardContent>
          </Card>
        ))}
      </div>

      <ProviderDialog
        open={dialogOpen}
        provider={editing}
        onClose={() => {
          setDialogOpen(false);
          setEditing(null);
        }}
      />
    </div>
  );
}
