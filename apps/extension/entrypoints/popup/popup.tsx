import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@sonasub/ui/components/card';

export function Popup() {
  return (
    <div className="w-80 bg-background p-3 text-foreground">
      <Card>
        <CardHeader>
          <CardTitle>SonaSub</CardTitle>
          <CardDescription>
            The extension entry point is ready; subtitling is not connected yet.
          </CardDescription>
        </CardHeader>
        <CardContent className="text-xs text-muted-foreground">
          Multi-tab handling, capture and recognition flows are left to later tasks.
        </CardContent>
      </Card>
    </div>
  );
}
